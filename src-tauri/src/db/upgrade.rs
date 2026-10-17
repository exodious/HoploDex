//! The one runner for creating a database and for upgrading one: a single
//! transaction for every step due, the foreign-key link check, and the test
//! hooks that exercise it (009 research.md §7, §14). Filled in by the runner
//! tasks of 009; for now it holds only the guard of the `upgrade-test-steps`
//! feature.

// `upgrade-test-steps` compiles in forced failures and test-only steps, so it
// must never reach a build that opens a user's files (009 research.md §14).
#[cfg(all(feature = "upgrade-test-steps", any(feature = "custom-protocol", feature = "e2e")))]
compile_error!(
    "the `upgrade-test-steps` feature is for `cargo test` builds only: never with `custom-protocol` \
     or `e2e`"
);

/// Test hooks (009 research.md §14): test-only steps, forced failures and
/// interruptions, registered per folder.
#[cfg(feature = "upgrade-test-steps")]
pub mod testing {}
