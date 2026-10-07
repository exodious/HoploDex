//! Research.md §5: Tauri counts every registered custom protocol as local,
//! and with no app manifest a local page may call every app command. So
//! `build.rs` gives `tauri_build` an app manifest naming every command
//! (`COMMANDS`), and `capabilities/default.json` allows each one on the
//! `main` window only. These read the shipped files and fail unless the
//! three lists agree, so a command can't be registered without a permission
//! or allowed without being registered, and the preview surface (window
//! `preview`) is never granted anything.

use std::collections::BTreeSet;
use std::path::PathBuf;

use hoplodex_lib::commands_list::COMMANDS;
use serde_json::Value;

fn manifest_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The command names `generate_handler!` in `main.rs` registers.
fn registered_commands() -> BTreeSet<String> {
    let source = std::fs::read_to_string(manifest_path("src/main.rs")).unwrap();
    let start = source.find("generate_handler![").expect("main.rs has generate_handler!")
        + "generate_handler![".len();
    let end = start + source[start..].find(']').expect("generate_handler! is closed");
    source[start..end]
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.rsplit("::").next().unwrap().to_string())
        .collect()
}

/// The permission Tauri generates for an app command.
fn permission_for(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

fn capabilities() -> Vec<(String, Value)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(manifest_path("capabilities")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        found.push((path.display().to_string(), value));
    }
    assert!(!found.is_empty(), "no capability files");
    found
}

fn permission_names(capability: &Value) -> Vec<String> {
    capability["permissions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| p.as_str().or_else(|| p["identifier"].as_str()).unwrap().to_string())
        .collect()
}

fn missing(label: &str, from: &BTreeSet<String>, to: &BTreeSet<String>) -> Vec<String> {
    from.difference(to).map(|name| format!("{name} (missing from {label})")).collect()
}

#[test]
fn commands_handler_and_capability_name_the_same_commands() {
    let listed: BTreeSet<String> = COMMANDS.iter().map(|c| c.to_string()).collect();
    assert_eq!(listed.len(), COMMANDS.len(), "COMMANDS names a command twice");
    let registered = registered_commands();

    let mut allowed = BTreeSet::new();
    for (_, capability) in capabilities() {
        for name in permission_names(&capability) {
            if name.starts_with("allow-") {
                allowed.insert(name);
            }
        }
    }
    let permissions: BTreeSet<String> = listed.iter().map(|c| permission_for(c)).collect();
    let registered_permissions: BTreeSet<String> =
        registered.iter().map(|c| permission_for(c)).collect();

    let mut problems = Vec::new();
    problems.extend(missing("COMMANDS", &registered, &listed));
    problems.extend(missing("generate_handler!", &listed, &registered));
    problems.extend(missing("capabilities/default.json", &permissions, &allowed));
    // A plain `allow-…` permission is an app command's; plugins' are prefixed
    // (`core:`, `dialog:`).
    problems.extend(missing("COMMANDS and generate_handler!", &allowed, &permissions));
    problems.extend(missing("capabilities/default.json", &registered_permissions, &allowed));
    assert!(problems.is_empty(), "the command lists disagree:\n  {}", problems.join("\n  "));
}

#[test]
fn every_capability_is_for_the_main_window_alone() {
    for (path, capability) in capabilities() {
        assert_eq!(
            capability["windows"],
            serde_json::json!(["main"]),
            "{path}: a capability must name exactly the main window (the preview surface gets none)"
        );
        assert!(
            capability.get("webviews").is_none(),
            "{path}: a capability must not name webviews"
        );
    }
}

#[test]
fn the_plugin_defaults_are_granted_in_one_capability_only() {
    for plugin_default in ["core:default", "dialog:default"] {
        let granted: Vec<String> = capabilities()
            .into_iter()
            .filter(|(_, c)| permission_names(c).iter().any(|p| p == plugin_default))
            .map(|(path, _)| path)
            .collect();
        assert_eq!(granted.len(), 1, "{plugin_default} appears in {granted:?}");
        assert!(granted[0].ends_with("default.json"), "{plugin_default} is in {}", granted[0]);
    }
}
