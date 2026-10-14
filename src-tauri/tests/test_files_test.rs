//! `Cargo.toml` turns off `autotests`, so a file in `tests/` that no `mod` line
//! in `integration.rs` names, and no `[[test]]` does, would compile nowhere and
//! its tests would silently never run (#77 step 9).

use std::fs;
use std::path::Path;

#[test]
fn every_test_file_is_in_the_integration_program_or_a_test_target_of_its_own() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(manifest.join("Cargo.toml")).unwrap();
    let root = fs::read_to_string(manifest.join("tests/integration.rs")).unwrap();

    let mut orphans = Vec::new();
    for entry in fs::read_dir(manifest.join("tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        if name == "integration" {
            continue;
        }
        let is_module = root.lines().any(|line| line.trim() == format!("mod {name};"));
        let is_target = cargo_toml.contains(&format!("name = \"{name}\""));
        if !is_module && !is_target {
            orphans.push(name);
        }
    }
    assert!(
        orphans.is_empty(),
        "these tests/*.rs files are never built: add a `mod` line to tests/integration.rs \
         or a [[test]] to Cargo.toml: {orphans:?}"
    );
}
