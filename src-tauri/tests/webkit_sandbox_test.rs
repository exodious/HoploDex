#![cfg(target_os = "linux")]
//! specs/007-document-preview FR-004, research.md §9: WebKitGTK's sandbox is
//! turned on only where a probe shows it works. This is the probe's decision
//! logic (tasks.md T034), without a web view: what the probe child did (exit
//! code, crash, time-out) and whether the computer is a container or Flatpak
//! decide whether `WEBKIT_FORCE_SANDBOX=1` is set.
//!
//! Assumed API, in `services::preview::sandbox_probe`:
//!
//! ```text
//! #[derive(Debug, Clone, Copy, PartialEq)]
//! pub enum ProbeOutcome { Exited(i32), Crashed, TimedOut }
//! #[derive(Debug)]
//! pub enum WebKitSandbox { On, Off { reason: String } }
//! pub const PROBE_TIMEOUT: Duration;        // 5 s
//! pub const SKIP_MARKERS: [&str; 3];        // the container and Flatpak files
//! pub fn decide(markers: &[&Path], probe: impl FnOnce() -> ProbeOutcome) -> WebKitSandbox;
//! ```
//!
//! `decide` looks for each marker path, returns `Off` with a reason naming
//! the one it found without calling `probe`, and otherwise calls `probe`
//! once and maps its outcome.

use std::cell::Cell;
use std::path::Path;
use std::time::Duration;

use hoplodex_lib::services::preview::sandbox_probe::{
    PROBE_TIMEOUT, ProbeOutcome, SKIP_MARKERS, WebKitSandbox, decide,
};
use tempfile::TempDir;

/// `decide` over `markers`, with the probe answering `outcome`; returns the
/// decision and how many times the probe ran.
fn decided(markers: &[&Path], outcome: ProbeOutcome) -> (WebKitSandbox, u32) {
    let ran = Cell::new(0);
    let decision = decide(markers, || {
        ran.set(ran.get() + 1);
        outcome
    });
    (decision, ran.get())
}

fn off_reason(decision: WebKitSandbox) -> String {
    match decision {
        WebKitSandbox::Off { reason } => reason,
        WebKitSandbox::On => panic!("the sandbox must be off"),
    }
}

#[test]
fn a_probe_that_exits_zero_turns_the_sandbox_on() {
    let (decision, ran) = decided(&[], ProbeOutcome::Exited(0));

    assert!(matches!(decision, WebKitSandbox::On), "{decision:?}");
    assert_eq!(ran, 1);
}

#[test]
fn a_non_zero_exit_turns_it_off_and_says_which() {
    for code in [1, 2, 101, 255, -1] {
        let (decision, ran) = decided(&[], ProbeOutcome::Exited(code));

        let reason = off_reason(decision);
        assert!(reason.contains(&code.to_string()), "exit {code}: {reason:?}");
        assert_eq!(ran, 1);
    }
}

#[test]
fn a_crash_turns_it_off_and_says_so() {
    // WebKit's g_error when bwrap can't run ends the probe by a signal.
    let (decision, _) = decided(&[], ProbeOutcome::Crashed);

    let reason = off_reason(decision).to_lowercase();
    assert!(reason.contains("crash"), "{reason:?}");
}

#[test]
fn a_time_out_turns_it_off_and_says_so() {
    let (decision, _) = decided(&[], ProbeOutcome::TimedOut);

    let reason = off_reason(decision).to_lowercase();
    assert!(reason.contains("time"), "{reason:?}");
}

#[test]
fn the_probe_is_given_five_seconds() {
    assert_eq!(PROBE_TIMEOUT, Duration::from_secs(5));
}

#[test]
fn each_reason_for_off_is_different_so_the_log_says_why() {
    let reasons: Vec<String> =
        [ProbeOutcome::Exited(1), ProbeOutcome::Crashed, ProbeOutcome::TimedOut]
            .into_iter()
            .map(|outcome| off_reason(decided(&[], outcome).0))
            .collect();

    assert!(reasons.iter().all(|r| !r.is_empty()));
    assert_ne!(reasons[0], reasons[1]);
    assert_ne!(reasons[1], reasons[2]);
    assert_ne!(reasons[0], reasons[2]);
}

#[test]
fn a_container_or_flatpak_marker_turns_it_off_without_probing() {
    let dir = TempDir::new().unwrap();
    for name in [".containerenv", ".dockerenv", ".flatpak-info"] {
        let marker = dir.path().join(name);
        std::fs::write(&marker, "").unwrap();

        // Even a probe that would say "works" is not asked.
        let (decision, ran) = decided(&[&marker], ProbeOutcome::Exited(0));

        let reason = off_reason(decision);
        assert!(reason.contains(name), "the reason names the marker: {reason:?}");
        assert_eq!(ran, 0, "{name}: the probe must not run");
    }
}

#[test]
fn one_marker_among_missing_ones_is_enough() {
    let dir = TempDir::new().unwrap();
    let missing_a = dir.path().join("missing-a");
    let present = dir.path().join(".flatpak-info");
    let missing_b = dir.path().join("missing-b");
    std::fs::write(&present, "").unwrap();

    let (decision, ran) = decided(&[&missing_a, &present, &missing_b], ProbeOutcome::Exited(0));

    assert!(matches!(decision, WebKitSandbox::Off { .. }), "{decision:?}");
    assert_eq!(ran, 0);
}

#[test]
fn with_no_marker_present_the_probe_decides() {
    let dir = TempDir::new().unwrap();
    let absent = [dir.path().join("a"), dir.path().join("b"), dir.path().join("c")];
    let markers: Vec<&Path> = absent.iter().map(|p| p.as_path()).collect();

    assert!(matches!(decided(&markers, ProbeOutcome::Exited(0)).0, WebKitSandbox::On));
    assert!(matches!(decided(&markers, ProbeOutcome::Exited(1)).0, WebKitSandbox::Off { .. }));
    assert!(matches!(decided(&markers, ProbeOutcome::Crashed).0, WebKitSandbox::Off { .. }));
    assert!(matches!(decided(&markers, ProbeOutcome::TimedOut).0, WebKitSandbox::Off { .. }));
}

#[test]
fn the_sandbox_is_never_on_unless_the_probe_exited_zero_and_no_marker_was_found() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join(".dockerenv");
    std::fs::write(&marker, "").unwrap();

    for outcome in [
        ProbeOutcome::Exited(0),
        ProbeOutcome::Exited(1),
        ProbeOutcome::Crashed,
        ProbeOutcome::TimedOut,
    ] {
        for markers in [vec![], vec![marker.as_path()]] {
            let (decision, _) = decided(&markers, outcome);
            let on = matches!(decision, WebKitSandbox::On);
            let expected = markers.is_empty() && matches!(outcome, ProbeOutcome::Exited(0));
            assert_eq!(on, expected, "{outcome:?} with {} marker(s)", markers.len());
        }
    }
}

#[test]
fn the_markers_the_app_looks_for_are_the_container_and_flatpak_files() {
    assert_eq!(SKIP_MARKERS, ["/run/.containerenv", "/.dockerenv", "/.flatpak-info"]);
}
