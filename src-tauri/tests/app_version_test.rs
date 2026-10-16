//! The version the app reports is the version released (008 FR-015,
//! research.md §16): `package.json`, `tauri.conf.json` and
//! `Cargo.toml` name the same one, and `APP_VERSION` is it.

use std::fs;
use std::path::Path;

use serde_json::Value;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn json_version(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let json: Value = serde_json::from_str(&text).unwrap();
    json["version"]
        .as_str()
        .unwrap_or_else(|| panic!("{} has no version", path.display()))
        .to_owned()
}

/// `[package] version` of `Cargo.toml`, read as text so the test needs no
/// TOML parser: the first `version = "…"` line after `[package]` and before
/// the next table.
fn cargo_version() -> String {
    let text = fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    let mut in_package = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && let Some(rest) = line.strip_prefix("version") {
            let value = rest.trim_start().strip_prefix('=').expect("version = \"…\"").trim();
            return value.trim_matches('"').to_owned();
        }
    }
    panic!("Cargo.toml has no [package] version");
}

#[test]
fn the_three_version_files_name_the_same_version() {
    let package = json_version(&manifest_dir().join("../package.json"));
    let tauri = json_version(&manifest_dir().join("tauri.conf.json"));
    let cargo = cargo_version();

    assert_eq!(package, tauri, "package.json and tauri.conf.json differ");
    assert_eq!(cargo, tauri, "Cargo.toml and tauri.conf.json differ");
}

#[test]
fn app_version_is_that_version() {
    assert_eq!(hoplodex_lib::APP_VERSION, cargo_version());
    assert_eq!(hoplodex_lib::APP_VERSION, json_version(&manifest_dir().join("tauri.conf.json")));
    assert!(!hoplodex_lib::APP_VERSION.is_empty());
}
