//! The test-only Cargo features that must never reach a shipped build each
//! refuse to compile there with a `compile_error!`. A shipped build is one
//! with Tauri's `custom-protocol`, which `tauri build` turns on without the
//! app's own feature, so each guard tests tauri-build's `not(dev)`. No test
//! build can compile a guard's failing case cheaply, so these read the
//! sources and fail if a guard is dropped or loses its `not(dev)`.

use std::path::PathBuf;

/// `(file, the guard's cfg)`; each must sit right above a `compile_error!`.
const GUARDS: &[(&str, &str)] = &[
    (
        "src/db/cipher.rs",
        r#"#[cfg(all(feature = "test-kdf", any(not(dev), feature = "e2e", not(debug_assertions))))]"#,
    ),
    (
        "src/db/upgrade.rs",
        r#"#[cfg(all(feature = "upgrade-test-steps", any(not(dev), feature = "e2e")))]"#,
    ),
    (
        "src/services/keyring.rs",
        r#"#[cfg(all(feature = "mock-keyring", not(feature = "e2e"), not(dev)))]"#,
    ),
];

#[test]
fn every_test_only_feature_refuses_to_compile_in_a_shipped_build() {
    for (file, cfg) in GUARDS {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            source.contains(&format!("{cfg}\ncompile_error!(")),
            "{file} no longer has the guard `{cfg}` above a `compile_error!`"
        );
    }
}
