// The one list of app commands, shared with the library (src/commands_list.rs).
include!("src/commands_list.rs");

fn main() {
    // Two cfg aliases for the Flatpak check variant (011 research.md §6, 011
    // T102): `flatpak-check` is the `e2e` build with the real routes, so the
    // opener and the keyring that `e2e` swaps for stand-ins stay what a shipped
    // build has. `mock_keyring`: the in-memory keyring, in a `mock-keyring` build
    // that is not the check variant. `app_opener`: `AppOpener`, in every build but
    // an `e2e` one that is not the check variant (`E2eOpener` logs instead).
    println!("cargo::rustc-check-cfg=cfg(mock_keyring)");
    println!("cargo::rustc-check-cfg=cfg(app_opener)");
    let has = |feature: &str| std::env::var_os(format!("CARGO_FEATURE_{feature}")).is_some();
    if has("MOCK_KEYRING") && !has("FLATPAK_CHECK") {
        println!("cargo::rustc-cfg=mock_keyring");
    }
    if !has("E2E") || has("FLATPAK_CHECK") {
        println!("cargo::rustc-cfg=app_opener");
    }
    // An app manifest makes Tauri refuse any command a window's capabilities
    // don't allow. Without one, every local page (and a registered custom
    // protocol counts as local) may call every app command (007 research.md §5).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
    // tauri-build links its Windows resource (the manifest asking for Common
    // Controls v6, the icon) into bins only. Examples that open a window
    // (pdf_surface_check) need the manifest too, or Windows refuses to start
    // them (STATUS_ENTRYPOINT_NOT_FOUND, TaskDialogIndirect).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
        println!("cargo:rustc-link-arg-examples={out}\\resource.lib");
    }
}
