//! FR-021 / SC-008: nothing in the app may reach the network, so the webview
//! gets a restrictive Content-Security-Policy and no network-capable Tauri
//! plugin or capability is enabled. These read the shipped configuration
//! itself — the CSP is enforced by the webview, not by code we can call.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

fn manifest_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_json(relative: &str) -> Value {
    let text = std::fs::read_to_string(manifest_path(relative))
        .unwrap_or_else(|e| panic!("could not read {relative}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{relative} is not valid JSON: {e}"))
}

/// A CSP as `directive -> sources`, from either config form (a string, or an
/// object of directive to string/array).
fn parse_csp(csp: &Value) -> BTreeMap<String, Vec<String>> {
    let mut directives = BTreeMap::new();
    match csp {
        Value::String(text) => {
            for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                let mut words = part.split_whitespace();
                let name = words.next().unwrap().to_string();
                directives.insert(name, words.map(str::to_string).collect());
            }
        }
        Value::Object(map) => {
            for (name, sources) in map {
                let sources = match sources {
                    Value::String(text) => text.split_whitespace().map(str::to_string).collect(),
                    Value::Array(items) => {
                        items.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()
                    }
                    other => panic!("unexpected CSP value for {name}: {other}"),
                };
                directives.insert(name.clone(), sources);
            }
        }
        other => panic!("`csp` must be a string or object, got {other}"),
    }
    directives
}

/// Sources that stay on the device: the app's own origin, inline data, the
/// local asset and IPC protocols. Anything else (a wildcard, a bare
/// `http:`/`https:`/`ws:`, or a remote host) is a way out.
const LOCAL_SOURCES: &[&str] = &[
    "'self'",
    "'none'",
    "data:",
    "blob:",
    "asset:",
    "ipc:",
    "http://asset.localhost",
    "http://ipc.localhost",
];

/// The dev server the frontend is served from under `tauri dev`.
const DEV_SERVER_SOURCES: &[&str] = &["http://localhost:1420", "ws://localhost:1420"];

fn assert_local_only(label: &str, directives: &BTreeMap<String, Vec<String>>, dev: bool) {
    for (name, sources) in directives {
        for source in sources {
            let allowed = LOCAL_SOURCES.contains(&source.as_str())
                || (dev && DEV_SERVER_SOURCES.contains(&source.as_str()))
                // Inline styles come from the UI library and React's `style`
                // attribute; inline *scripts* stay forbidden in production.
                || (source == "'unsafe-inline'" && (name.starts_with("style") || dev));
            assert!(allowed, "{label}: `{name}` allows {source}, which could leave the device");
        }
    }
}

#[test]
fn the_webview_has_a_restrictive_csp() {
    let config = read_json("tauri.conf.json");
    let csp = &config["app"]["security"]["csp"];
    assert!(!csp.is_null(), "app.security.csp must not be null: a null CSP allows everything");

    let directives = parse_csp(csp);
    assert_eq!(
        directives.get("default-src").map(Vec::as_slice),
        Some(&["'self'".to_string()][..]),
        "default-src must be 'self' alone"
    );
    assert_local_only("csp", &directives, false);

    let connect = directives.get("connect-src").expect("connect-src must be stated explicitly");
    assert!(
        connect.iter().all(|s| ["'self'", "ipc:", "http://ipc.localhost"].contains(&s.as_str())),
        "connect-src may only reach the app itself and its IPC, got {connect:?}"
    );

    for forbidden in ["script-src", "default-src"] {
        let sources = directives.get(forbidden).map(Vec::as_slice).unwrap_or_default();
        assert!(
            !sources.iter().any(|s| s == "'unsafe-inline'" || s == "'unsafe-eval'"),
            "{forbidden} must not allow inline or eval'd scripts"
        );
    }
    assert!(
        directives.get("object-src").is_some_and(|s| s == &["'none'"]),
        "plugins (object-src) must be disabled"
    );
}

#[test]
fn photos_and_thumbnails_can_still_render() {
    // Photos reach the page as data: URLs (src/lib/bytes.ts); generic
    // per-type thumbnails are drawn inline as SVG, needing no image source.
    let config = read_json("tauri.conf.json");
    let directives = parse_csp(&config["app"]["security"]["csp"]);

    let images = directives.get("img-src").expect("img-src must be stated explicitly");
    for needed in ["'self'", "data:", "blob:"] {
        assert!(images.iter().any(|s| s == needed), "img-src must allow {needed}: {images:?}");
    }
}

#[test]
fn a_dev_csp_only_adds_the_local_dev_server() {
    let config = read_json("tauri.conf.json");
    let dev = &config["app"]["security"]["devCsp"];
    if dev.is_null() {
        return;
    }
    let directives = parse_csp(dev);
    assert_local_only("devCsp", &directives, true);
}

#[test]
fn no_network_capable_plugin_is_a_dependency_or_has_a_permission() {
    // Plugins whose purpose is to make network requests or open sockets.
    const NETWORK_PLUGINS: &[&str] =
        &["http", "websocket", "updater", "upload", "deep-link", "sql", "stronghold"];

    let manifest = std::fs::read_to_string(manifest_path("Cargo.toml")).unwrap();
    for plugin in NETWORK_PLUGINS {
        assert!(
            !manifest.contains(&format!("tauri-plugin-{plugin}")),
            "tauri-plugin-{plugin} can reach the network; HoploDex has no network feature (FR-021)"
        );
    }

    for entry in std::fs::read_dir(manifest_path("capabilities")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let capability: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for permission in capability["permissions"].as_array().into_iter().flatten() {
            let name = permission.as_str().or_else(|| permission["identifier"].as_str()).unwrap();
            for plugin in NETWORK_PLUGINS {
                assert!(
                    !name.starts_with(&format!("{plugin}:")),
                    "{}: permission {name} grants network access",
                    path.display()
                );
            }
            assert!(
                !name.starts_with("opener:allow-open-url"),
                "{}: the webview must not be able to open URLs ({name})",
                path.display()
            );
        }
    }
}

/// specs/007-document-preview research.md §13: the main web view gets no
/// frames, workers, wasm or eval from the document preview (the PDF is shown
/// in a separate surface), so a later change can't loosen the CSP to bring a
/// PDF engine into it.
#[test]
fn the_csp_still_forbids_frames_workers_wasm_and_eval() {
    let config = read_json("tauri.conf.json");
    let directives = parse_csp(&config["app"]["security"]["csp"]);

    assert_eq!(
        directives.get("frame-src").map(Vec::as_slice),
        Some(&["'none'".to_string()][..]),
        "frame-src must stay 'none'"
    );
    // Workers fall back to script-src, then to default-src ('self' alone), so
    // a `worker-src` may only be as strict.
    if let Some(workers) = directives.get("worker-src") {
        assert!(
            workers.iter().all(|s| s == "'self'" || s == "'none'"),
            "worker-src must not loosen default-src 'self', got {workers:?}"
        );
        assert!(!workers.iter().any(|s| s == "blob:" || s == "data:"), "no worker from a URL");
    }
    assert_eq!(directives.get("default-src").map(Vec::as_slice), Some(&["'self'".to_string()][..]));
    for directive in ["script-src", "default-src"] {
        let sources = directives.get(directive).map(Vec::as_slice).unwrap_or_default();
        for forbidden in ["'unsafe-eval'", "'wasm-unsafe-eval'"] {
            assert!(
                !sources.iter().any(|s| s == forbidden),
                "{directive} must not allow {forbidden}"
            );
        }
    }
    for directive in ["child-src", "object-src"] {
        if let Some(sources) = directives.get(directive) {
            assert_eq!(sources, &["'none'".to_string()], "{directive} must not allow a frame");
        }
    }
}
