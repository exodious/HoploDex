// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_menu;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use hoplodex_lib::app_dirs;
#[cfg(feature = "e2e")]
use hoplodex_lib::commands::documents::E2eOpener;
#[cfg(not(feature = "e2e"))]
use hoplodex_lib::commands::documents::{AppOpener, opener_route};
use hoplodex_lib::commands::documents::{
    OPENED_DOCUMENTS_DIR, Opener, clear_opened_documents_cache,
};
use hoplodex_lib::commands::preview::{AppSurfaces, PreviewEnv};
use hoplodex_lib::db;
use hoplodex_lib::platform::{self, SystemEvent};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::consent::Consent;
#[cfg(not(feature = "e2e"))]
use hoplodex_lib::services::consent::DialogConsent;
#[cfg(feature = "e2e")]
use hoplodex_lib::services::consent::E2eConsent;
use hoplodex_lib::services::import_reader::reader as import_helper;
use hoplodex_lib::services::keyring::Keyring;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::main_navigation;
use hoplodex_lib::services::preview::availability::{PdfAvailabilityState, decide_at_startup};
#[cfg(target_os = "linux")]
use hoplodex_lib::services::preview::sandbox_probe;
use hoplodex_lib::services::preview::{helper as render_helper, protocol_handler, surface};
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::session::{Session, lifecycle};
use hoplodex_lib::window_controls;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// What the frontend is sent when the user closes the window or quits: it
/// asks about unsaved changes, then calls `quit_application` (003 research.md
/// §17).
const QUIT_REQUESTED: &str = "app:quit-requested";

/// The label of the main window, the only web view the frontend's events go to
/// (the PDF preview's child web view has no listener and gets none; 010 T072).
const MAIN_WINDOW: &str = "main";

/// The OS is shutting down or ending the application, so its exit request
/// is let through rather than asked about (003 research.md §17).
static OS_ENDING: AtomicBool = AtomicBool::new(false);

/// Asks the frontend to quit: the unsaved-changes question first, then
/// `quit_application` (003 research.md §17).
fn request_quit(app: &AppHandle) {
    if let Err(err) = app.emit_to(MAIN_WINDOW, QUIT_REQUESTED, ()) {
        log::warn!("could not emit {QUIT_REQUESTED}: {err}");
    }
}

/// The OS ends the application (003 FR-039): the open database closes at once,
/// keeping unsaved input as pending changes before the key is cleared and
/// decrypted document copies are deleted, with no backup. Safe to run twice
/// at once (the second waits for the first's close).
fn close_for_the_os(app: &AppHandle) {
    OS_ENDING.store(true, Ordering::SeqCst);
    // The session's close ends the preview first (the PDF surface is closed
    // and the helper killed), then the copies the PDF viewer left on macOS
    // are swept (007 FR-003a, research.md §7).
    lifecycle::will_shut_down(&app.state::<Session>(), &app.state::<MachineSettings>());
    app.state::<MachineSettings>().sweep_hold_leftovers();
}

/// [`close_for_the_os`], then the app exits.
fn end_for_the_os(app: &AppHandle) {
    close_for_the_os(app);
    app.exit(0);
}

/// A session shutdown (SIGTERM, SIGHUP) or Ctrl+C asks the app to exit. Left
/// alone the process would just die, skipping the exit handler below, so route
/// them through the OS-ending path: pending changes are kept, and decrypted
/// document copies must not outlive the session (001 FR-035; 003 FR-039).
#[cfg(unix)]
fn exit_on_termination_signals(app: tauri::AppHandle) {
    use tokio::signal::unix::{SignalKind, signal};

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
        // The close waits on other threads, so it runs off the runtime.
        tauri::async_runtime::spawn_blocking(move || end_for_the_os(&app));
    });
}

/// Hands the OS's notices to the session (003 research.md §14).
fn handle_system_events(app: AppHandle, events: mpsc::Receiver<SystemEvent>) {
    let spawned = thread::Builder::new().name("system-events".into()).spawn(move || {
        for event in events {
            let session = app.state::<Session>();
            let machine = app.state::<MachineSettings>();
            match event {
                // The OS waits for `ack` to be dropped, once the lock is done.
                SystemEvent::WillSleep { ack } => {
                    lifecycle::will_sleep(&session, &machine);
                    drop(ack);
                }
                SystemEvent::Woke => lifecycle::finish_on_wake(&session, &machine),
                // Its lock makes a backup, which can take a while. Closing a
                // laptop's lid locks the screen and then sleeps it, and that
                // sleep must reach the close under way to finish it at once
                // (003 research.md §14), so the lock runs on its own thread.
                SystemEvent::ScreenLocked => {
                    let app = app.clone();
                    let spawned =
                        thread::Builder::new().name("screen-lock".into()).spawn(move || {
                            let session = app.state::<Session>();
                            lifecycle::screen_locked(&session, &app.state::<MachineSettings>());
                        });
                    if let Err(err) = spawned {
                        log::error!("could not lock for the screen lock: {err}");
                    }
                }
                SystemEvent::ScreenUnlocked => {}
                SystemEvent::WillShutDown { ack } => {
                    end_for_the_os(&app);
                    drop(ack);
                }
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start handling system events: {err}");
    }
}

/// The idle lock's 1 s tick (003 research.md §15).
fn tick_idle_clock(app: AppHandle) {
    let spawned = thread::Builder::new().name("idle-clock".into()).spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            lifecycle::idle_tick(&app.state::<Session>(), &app.state::<MachineSettings>());
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the idle clock: {err}");
    }
}

/// What this run may reach on the filesystem (011 research.md §5): inside a
/// Flatpak, the grants `/.flatpak-info` lists; everywhere else, and on macOS
/// and Windows always, everything.
#[cfg(target_os = "linux")]
fn read_reach(app: &AppHandle) -> Reach {
    use hoplodex_lib::services::sandbox::{FLATPAK_INFO, PortalHostPaths, UserDirs};
    use std::path::{Path, PathBuf};

    let info = Path::new(FLATPAK_INFO);
    // Outside a Flatpak nothing below is read, so a run in a sandbox of the
    // E2E harness never looks at the developer's own folders.
    if !info.exists() {
        return Reach::Unconfined;
    }
    let host_paths = Arc::new(PortalHostPaths::new());
    let Some(home) =
        app_dirs::home_dir(app).ok().or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    else {
        log::error!("the home folder is unknown, so nothing is within reach");
        return Reach::flatpak(Vec::new(), Vec::new(), PathBuf::new(), host_paths);
    };
    // `user-dirs.dirs` is in the config home (`XDG_CONFIG_HOME`; Flatpak points
    // it at the app's own folder and binds the host's file into it).
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let user_dirs = UserDirs::read(&config_home, &home);
    Reach::detect(info, &home, &user_dirs, host_paths)
}

#[cfg(not(target_os = "linux"))]
fn read_reach(_app: &AppHandle) -> Reach {
    Reach::Unconfined
}

fn main() {
    // The render helper, the import helper and the sandbox probe are this executable started
    // again (007 research.md §9, §11; 008 research.md §8): before any Tauri, keyring, session
    // or logging setup, so they have none of them, and they never return here.
    let args: Vec<String> =
        std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    #[cfg(target_os = "linux")]
    if args.first().map(String::as_str) == Some(sandbox_probe::ARGUMENT) {
        sandbox_probe::run_child_probe();
    }
    if args.first().map(String::as_str) == Some(render_helper::ARGUMENT) {
        render_helper::run(&args);
    }
    if args.first().map(String::as_str) == Some(import_helper::ARGUMENT) {
        import_helper::run(&args);
    }
    // The tests run the helpers above from a `test-kdf` build, but the app
    // itself must not start in one: a database it made would have the tests'
    // KDF (db/cipher.rs). db/cipher.rs already keeps the feature out of
    // release, E2E and shipped builds; this covers `cargo run` and `tauri dev`.
    // `upgrade-test-steps` (which `test-kdf` turns on) compiles in forced
    // upgrade failures and test-only steps (009 research.md §14).
    if cfg!(feature = "test-kdf") || cfg!(feature = "upgrade-test-steps") {
        eprintln!(
            "HoploDex can't start in a build with the test-kdf feature or the upgrade-test-steps feature, which are for the tests only."
        );
        std::process::exit(1);
    }
    // The log goes to standard error from here on (the helpers above have none).
    hoplodex_lib::logging::init();
    // WebKit's sandbox, on only where a probe shows it works, and decided
    // here because the variable that turns it on must be set before the
    // first web context exists (007 research.md §9).
    #[cfg(target_os = "linux")]
    let webkit_sandbox = sandbox_probe::decide_and_apply();
    let builder = tauri::Builder::default();
    // The E2E suite's WebDriver server, on 127.0.0.1 at
    // `TAURI_WEBDRIVER_PORT` (#29). It has no authentication and runs any
    // script in the page, so it must never reach a shipped build.
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    let app = builder
        // The PDF surface's documents, from memory (007 research.md §4).
        .register_asynchronous_uri_scheme_protocol(
            protocol_handler::SCHEME,
            |context, request, responder| {
                let session = context.app_handle().state::<Session>().inner().clone();
                // Only the PDF surface's web view may ask (007 contracts/
                // tauri-commands.md "The `hdpreview` protocol").
                let webview_label = context.webview_label().to_owned();
                // The handler takes the session's lock, so not on the thread
                // the web view asks from.
                tauri::async_runtime::spawn_blocking(move || {
                    responder.respond(protocol_handler::handle(&session, &webview_label, &request));
                });
            },
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            {
                log::info!("{}", webkit_sandbox.describe());
                app.manage(webkit_sandbox);
            }
            // Backstop for a crash or forced kill: decrypted document copies
            // a previous session couldn't clean up on exit (001 FR-035).
            clear_opened_documents_cache(app.handle());
            #[cfg(unix)]
            exit_on_termination_signals(app.handle().clone());
            db::cipher::silence_cipher_log();
            // No database is open at startup: the user chooses one and gives
            // its passphrase (specs/003-database-protection-management).
            let opened_documents = app_dirs::cache_dir(app.handle())?.join(OPENED_DOCUMENTS_DIR);
            // What this run may reach: read once, and the one value the
            // commands and the session both ask (011 research.md §5).
            let reach = Arc::new(read_reach(app.handle()));
            log::info!("{}", reach.describe());
            app.manage(reach.clone());
            app.manage(Session::new(
                Arc::new(app.handle().clone()),
                Some(opened_documents),
                reach.clone(),
            ));
            // Saved passphrases (003 FR-017); the keyring itself is first asked
            // about when the chooser needs to know whether it is there.
            let machine = MachineSettings::load(&app_dirs::config_dir(app.handle())?)?
                .with_keyring(Keyring::system());
            // A backup a crash or forced quit cut short (003 research.md §7).
            backups::sweep_unfinished(&machine);
            // Copies the macOS PDF viewer left behind in a run that couldn't
            // delete them (007 FR-003a, research.md §17). A hold for another
            // version is cleared in `decide_at_startup` below.
            machine.sweep_hold_leftovers();
            // Whether PDFs are previewed on this computer in this run: this
            // version's hold, then the web view's check, before anything is
            // shown (007 research.md §7). The surface's browser data folder from
            // a previous run goes too (007 research.md §6).
            surface::clear_data_directory(&app_dirs::preview_webview_data_dir(app.handle())?);
            let availability = PdfAvailabilityState::new(decide_at_startup(&machine));
            app.manage(machine);
            app.manage(availability.clone());
            // The preview commands: the helper is this executable.
            app.manage(PreviewEnv::new(
                std::env::current_exe()?,
                Arc::new(AppSurfaces::new(app.handle().clone())),
                Arc::new(availability),
            ));
            // The native confirmation before a document goes to another app,
            // and the launcher that hands it on. An E2E build answers from
            // its environment and logs the hand-over instead (007 research.md
            // §16).
            #[cfg(not(feature = "e2e"))]
            {
                app.manage::<Arc<dyn Consent>>(Arc::new(DialogConsent::new(app.handle().clone())));
                app.manage::<Arc<dyn Opener>>(Arc::new(AppOpener::new(
                    app.handle().clone(),
                    opener_route(&reach),
                )));
            }
            #[cfg(feature = "e2e")]
            {
                app.manage::<Arc<dyn Consent>>(Arc::new(E2eConsent));
                app.manage::<Arc<dyn Opener>>(Arc::new(E2eOpener));
            }
            // Sleep, wake, screen lock and shutdown (003 FR-037, FR-038), and
            // the idle lock (003 FR-034).
            let (sender, events) = mpsc::channel();
            platform::spawn_listener(sender);
            handle_system_events(app.handle().clone(), events);
            tick_idle_clock(app.handle().clone());
            // The main window is built here rather than from tauri.conf.json
            // so its web view gets a data folder of its own, apart from the
            // preview surface's (007 research.md §6). Same title and sizes as the
            // config gave it.
            //
            // It may navigate only to the app's own page, and never opens
            // another window (#73, 007 research.md §6): the CSP governs what the
            // page fetches, not where the window goes. The dev server's
            // origin counts only in a development run. On Windows a request
            // filter backs the navigation handler, which WebView2 may answer
            // only after the request has gone out (main_navigation).
            let dev_url = if tauri::is_dev() { app.config().build.dev_url.clone() } else { None };
            let origins = main_navigation::app_origins(cfg!(windows), dev_url.as_ref());
            #[cfg(windows)]
            let filter_origins = origins.clone();
            let _main = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::default())
                .on_navigation(move |url| main_navigation::navigation_allowed(&origins, url))
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .title("HoploDex")
                .inner_size(1200.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .data_directory(app_dirs::main_webview_data_dir(app.handle())?)
                .build()?;
            #[cfg(windows)]
            main_navigation::refuse_foreign_documents(&_main, filter_origins)?;
            // The native half of the browser controls (008 FR-010 to FR-012,
            // 008 research.md §13): in release and E2E builds only, and a
            // failure leaves the web view's own menu rather than keeping
            // the app from starting.
            if let Err(err) = window_controls::apply(&_main, false) {
                log::error!("the native browser controls could not be installed: {err}");
            }
            // The macOS application menu, with its About item opening
            // HoploDex's own About screen (010 research.md §11). Linux and
            // Windows set no menu. A failure leaves Tauri's default menu.
            #[cfg(target_os = "macos")]
            match app_menu::build(app.handle()) {
                Ok(menu) => {
                    if let Err(err) = app.set_menu(menu) {
                        log::error!("the application menu could not be set: {err}");
                    }
                }
                Err(err) => log::error!("the application menu could not be built: {err}"),
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            // Only the About item is the app's own; the predefined items act
            // by themselves (010 contracts/tauri-commands.md).
            if let Some(name) = app_menu::requested_event(event.id().as_ref())
                && let Err(err) = app.emit_to(MAIN_WINDOW, name, ())
            {
                log::warn!("could not emit {name}: {err}");
            }
        })
        .on_window_event(|window, event| {
            // Closing the window is a request: unsaved changes are asked
            // about first, and the database is closed normally.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                request_quit(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            hoplodex_lib::commands::databases::get_chooser_state,
            hoplodex_lib::commands::databases::create_database,
            hoplodex_lib::commands::databases::open_database,
            hoplodex_lib::commands::databases::upgrade_database,
            hoplodex_lib::commands::databases::cancel_upgrade,
            hoplodex_lib::commands::databases::close_database,
            hoplodex_lib::commands::databases::quit_application,
            hoplodex_lib::commands::databases::remove_recent_database,
            hoplodex_lib::commands::databases::locate_database,
            hoplodex_lib::commands::databases::get_database_status,
            hoplodex_lib::commands::databases::dismiss_note,
            hoplodex_lib::commands::databases::update_backup_settings,
            hoplodex_lib::commands::databases::skip_backup,
            hoplodex_lib::commands::databases::update_lock_settings,
            hoplodex_lib::commands::databases::lock_database,
            hoplodex_lib::commands::databases::get_document_opening,
            hoplodex_lib::commands::databases::set_document_opening,
            hoplodex_lib::commands::databases::stage_pending_changes,
            hoplodex_lib::commands::databases::resolve_pending_changes,
            hoplodex_lib::commands::databases::note_activity,
            hoplodex_lib::commands::databases::set_idle_paused,
            hoplodex_lib::commands::databases::save_passphrase,
            hoplodex_lib::commands::databases::forget_saved_passphrase,
            hoplodex_lib::commands::backups::list_backups,
            hoplodex_lib::commands::backups::restore_backup,
            hoplodex_lib::commands::backups::check_backup,
            hoplodex_lib::commands::backups::delete_all_backups,
            hoplodex_lib::commands::backups::change_passphrase,
            hoplodex_lib::commands::firearms::create_firearm,
            hoplodex_lib::commands::firearms::update_firearm,
            hoplodex_lib::commands::firearms::dispose_firearm,
            hoplodex_lib::commands::firearms::reverse_disposition,
            hoplodex_lib::commands::firearms::delete_firearm,
            hoplodex_lib::commands::firearms::get_firearm,
            hoplodex_lib::commands::firearms::list_firearms,
            hoplodex_lib::commands::accessories::create_accessory,
            hoplodex_lib::commands::accessories::update_accessory,
            hoplodex_lib::commands::accessories::get_accessory,
            hoplodex_lib::commands::accessories::list_accessories,
            hoplodex_lib::commands::accessories::dispose_accessory,
            hoplodex_lib::commands::accessories::reverse_accessory_disposition,
            hoplodex_lib::commands::accessories::delete_accessory,
            hoplodex_lib::commands::mounts::mount_record,
            hoplodex_lib::commands::mounts::list_mount_candidates,
            hoplodex_lib::commands::entries::list_accessory_kinds,
            hoplodex_lib::commands::entries::suggest_entries,
            hoplodex_lib::commands::entries::settle_entry,
            hoplodex_lib::commands::entries::list_action_types,
            hoplodex_lib::commands::entries::list_firearm_types,
            hoplodex_lib::commands::entries::list_registration_classes,
            hoplodex_lib::commands::insurance::list_insurance_policies,
            hoplodex_lib::commands::insurance::create_insurance_policy,
            hoplodex_lib::commands::insurance::update_insurance_policy,
            hoplodex_lib::commands::insurance::get_policy_deletion_impact,
            hoplodex_lib::commands::insurance::delete_insurance_policy,
            hoplodex_lib::commands::insurance::assign_firearm_coverage,
            hoplodex_lib::commands::insurance::assign_accessory_coverage,
            hoplodex_lib::commands::insurance::get_value_summary,
            hoplodex_lib::commands::photos::list_photos,
            hoplodex_lib::commands::photos::add_photo,
            hoplodex_lib::commands::photos::add_photo_from_path,
            hoplodex_lib::commands::photos::get_photo_thumbnail,
            hoplodex_lib::commands::photos::get_photo_original,
            hoplodex_lib::commands::photos::set_thumbnail_photo,
            hoplodex_lib::commands::photos::delete_photo,
            hoplodex_lib::commands::documents::list_document_types,
            hoplodex_lib::commands::documents::list_documents,
            hoplodex_lib::commands::documents::add_document,
            hoplodex_lib::commands::documents::add_document_from_path,
            hoplodex_lib::commands::documents::open_document,
            hoplodex_lib::commands::documents::delete_document,
            hoplodex_lib::commands::preview::open_preview,
            hoplodex_lib::commands::preview::close_preview,
            hoplodex_lib::commands::preview::set_preview_bounds,
            hoplodex_lib::commands::preview::focus_preview,
            hoplodex_lib::commands::preview::render_preview_page,
            hoplodex_lib::commands::import_export::get_export_scope,
            hoplodex_lib::commands::import_export::export_collection,
            hoplodex_lib::commands::import_export::import_collection,
            hoplodex_lib::commands::import_export::cancel_import,
            hoplodex_lib::commands::import_export::resolve_import_conflicts,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    // Every macOS `terminate:` (⌘Q, the application menu's and the Dock's
    // Quit, an Apple Event quit) is a request, like the window's close
    // button (003 research.md §17, #107). tao ends the app from
    // `applicationWillTerminate:` with no `RunEvent::ExitRequested`, so the
    // delegate is asked first. A quit the OS sends as a session ends (its
    // Apple Event names a log out, restart or shut down, or the power-off
    // notice came first) must not be cancelled, or the log out is: it is
    // answered NSTerminateLater, closed off the main thread (003 FR-039), then
    // replied to. The app's own exit with a code (`quit_application`,
    // `end_for_the_os`) stops the loop and never calls `terminate:`.
    #[cfg(target_os = "macos")]
    {
        let handle = app.handle().clone();
        platform::ask_before_terminate(move |source| {
            if source == platform::QuitSource::User && !OS_ENDING.load(Ordering::SeqCst) {
                request_quit(&handle);
                return platform::Termination::Cancel;
            }
            // The close waits on other threads, some of which wait on this
            // one, so it can't run here; the reply is queued back to it.
            let app = handle.clone();
            let spawned = thread::Builder::new().name("os-terminate".into()).spawn(move || {
                close_for_the_os(&app);
                platform::reply_to_terminate(true);
            });
            match spawned {
                Ok(_) => platform::Termination::Later,
                Err(err) => {
                    log::error!("could not close for the OS ending the app: {err}");
                    platform::Termination::Now
                }
            }
        });
    }
    app.run(|app, event| match event {
        // An exit without a code is the user quitting (the application
        // menu, the last window): a request, like closing the window.
        // `quit_application` and the termination signals exit with one.
        // One the OS asks for as it shuts down is let through.
        RunEvent::ExitRequested { code: None, api, .. } if !OS_ENDING.load(Ordering::SeqCst) => {
            api.prevent_exit();
            request_quit(app);
        }
        // Decrypted copies of opened documents live only as long as the
        // session (001 FR-035); the startup sweep covers abnormal exits.
        RunEvent::Exit => {
            clear_opened_documents_cache(app);
            // Not there when the exit is a setup that failed.
            if let Some(machine) = app.try_state::<MachineSettings>() {
                machine.sweep_hold_leftovers();
            }
        }
        _ => {}
    });
}
