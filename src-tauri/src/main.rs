// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::db::{self, DbHandle};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Temporary document copies from a previous session's
            // `open_document` calls; nothing depends on them surviving.
            if let Ok(cache_dir) = app.path().app_cache_dir() {
                let _ = std::fs::remove_dir_all(
                    cache_dir.join(hoplodex_lib::commands::documents::OPENED_DOCUMENTS_DIR),
                );
            }
            let conn = db::init_app_db(app.handle())?;
            app.manage(DbHandle(Mutex::new(conn)));
            app.manage(ImportSessionStore::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hoplodex_lib::commands::firearms::create_firearm,
            hoplodex_lib::commands::firearms::update_firearm,
            hoplodex_lib::commands::firearms::dispose_firearm,
            hoplodex_lib::commands::firearms::delete_firearm,
            hoplodex_lib::commands::firearms::get_firearm,
            hoplodex_lib::commands::firearms::list_firearms,
            hoplodex_lib::commands::insurance::list_insurance_policies,
            hoplodex_lib::commands::insurance::create_insurance_policy,
            hoplodex_lib::commands::insurance::update_insurance_policy,
            hoplodex_lib::commands::insurance::delete_insurance_policy,
            hoplodex_lib::commands::insurance::assign_firearm_coverage,
            hoplodex_lib::commands::insurance::get_value_summary,
            hoplodex_lib::commands::photos::list_photos,
            hoplodex_lib::commands::photos::add_photo,
            hoplodex_lib::commands::photos::add_photo_from_path,
            hoplodex_lib::commands::photos::get_photo_thumbnail,
            hoplodex_lib::commands::photos::get_photo_original,
            hoplodex_lib::commands::photos::set_thumbnail_photo,
            hoplodex_lib::commands::photos::delete_photo,
            hoplodex_lib::commands::photos::get_generic_thumbnail,
            hoplodex_lib::commands::documents::list_documents,
            hoplodex_lib::commands::documents::add_document,
            hoplodex_lib::commands::documents::add_document_from_path,
            hoplodex_lib::commands::documents::get_document,
            hoplodex_lib::commands::documents::open_document,
            hoplodex_lib::commands::documents::delete_document,
            hoplodex_lib::commands::import_export::export_collection,
            hoplodex_lib::commands::import_export::import_collection,
            hoplodex_lib::commands::import_export::resolve_import_conflicts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
