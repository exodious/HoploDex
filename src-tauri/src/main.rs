// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hoplodex_lib::commands::documents::clear_opened_documents_cache;
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::db;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::operations::Operations;
use hoplodex_lib::session::Session;
use tauri::{Manager, RunEvent};

/// A session shutdown (SIGTERM, SIGHUP) or Ctrl+C asks the app to exit. Left
/// alone the process would just die, skipping the exit handler below, so route
/// them through the normal exit path: decrypted document copies must not
/// outlive the session even when the OS ends it (FR-035).
#[cfg(unix)]
fn exit_on_termination_signals(app: tauri::AppHandle) {
    use tokio::signal::unix::{signal, SignalKind};

    tauri::async_runtime::spawn(async move {
        let (Ok(mut terminate), Ok(mut hangup), Ok(mut interrupt)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::hangup()),
            signal(SignalKind::interrupt()),
        ) else {
            return;
        };
        tokio::select! {
            _ = terminate.recv() => {}
            _ = hangup.recv() => {}
            _ = interrupt.recv() => {}
        }
        app.exit(0);
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Backstop for a crash or forced kill: decrypted document copies
            // a previous session couldn't clean up on exit (FR-035).
            clear_opened_documents_cache(app.handle());
            #[cfg(unix)]
            exit_on_termination_signals(app.handle().clone());
            db::cipher::silence_cipher_log();
            // No database is open at startup: the user chooses one and gives
            // its passphrase (specs/003-database-protection-management).
            app.manage(Session::default());
            app.manage(Operations::default());
            app.manage(MachineSettings::load(&app.path().app_config_dir()?)?);
            app.manage(ImportSessionStore::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hoplodex_lib::commands::databases::get_chooser_state,
            hoplodex_lib::commands::databases::create_database,
            hoplodex_lib::commands::databases::open_database,
            hoplodex_lib::commands::databases::get_database_status,
            hoplodex_lib::commands::databases::dismiss_note,
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
            hoplodex_lib::commands::documents::list_documents,
            hoplodex_lib::commands::documents::add_document,
            hoplodex_lib::commands::documents::add_document_from_path,
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
