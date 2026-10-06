fn main() {
    tauri_build::build();
    // tauri-build links its Windows resource (the manifest asking for Common
    // Controls v6, the icon) into bins only. Examples that open a window
    // (pdf_spike) need the manifest too, or Windows refuses to start them
    // (STATUS_ENTRYPOINT_NOT_FOUND, TaskDialogIndirect).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
        println!("cargo:rustc-link-arg-examples={out}\\resource.lib");
    }
}
