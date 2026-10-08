// The one list of app commands, shared with the library (src/commands_list.rs).
include!("src/commands_list.rs");

fn main() {
    // An app manifest makes Tauri refuse any command a window's capabilities
    // don't allow. Without one, every local page (and a registered custom
    // protocol counts as local) may call every app command (research.md §5).
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
