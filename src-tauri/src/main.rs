// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use hoplodex_lib::commands::documents::clear_opened_documents_cache;
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::db::{self, DbHandle};
use tauri::{Manager, RunEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Backstop for a crash or forced kill: decrypted document copies
            // a previous session couldn't clean up on exit (FR-035).
            clear_opened_documents_cache(app.handle());
            let conn = db::init_app_db(app.handle())?;
            app.manage(DbHandle(Mutex::new(conn)));
            app.manage(ImportSessionStore::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hoplodex_lib::commands::firearms::create_firearm,
            hoplodex_lib::commands::firearms::update_firearm,
            hoplodex_lib::commands::firearms::dispose_firearm,
            hoplodex_lib::commands::firearms::reverse_disposition,
            hoplodex_lib::commands::firearms::delete_firearm,
            hoplodex_lib::commands::firearms::get_firearm,
            hoplodex_lib::commands::firearms::list_firearms,
            hoplodex_lib::commands::insurance::list_insurance_policies,
            hoplodex_lib::commands::insurance::create_insurance_policy,
            hoplodex_lib::commands::insurance::update_insurance_policy,
            hoplodex_lib::commands::insurance::get_policy_deletion_impact,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Decrypted copies of opened documents live only as long as the
            // session (FR-035); the startup sweep covers abnormal exits.
            if let RunEvent::Exit = event {
                clear_opened_documents_cache(app);
            }
        });
}
