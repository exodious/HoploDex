// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use hoplodex_lib::db::{self, DbHandle};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let conn = db::init_app_db(app.handle())?;
            app.manage(DbHandle(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hoplodex_lib::commands::firearms::create_firearm,
            hoplodex_lib::commands::firearms::update_firearm,
            hoplodex_lib::commands::firearms::dispose_firearm,
            hoplodex_lib::commands::firearms::delete_firearm,
            hoplodex_lib::commands::firearms::get_firearm,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
