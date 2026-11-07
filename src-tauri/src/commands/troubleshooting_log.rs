//! The troubleshooting log's commands (013 contracts/tauri-commands.md). None
//! touches a database, so all three are unscoped. The page gets a state and a
//! path, never the file's contents (013 FR-024).

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::commands::documents::{OpenFailure, Opener};
use crate::commands::error::CommandError;
use crate::logging;
use crate::models::troubleshooting_log::{LogState, TroubleshootingLog};
use crate::services::machine_settings::MachineSettings;

/// The state changes as the contract's table says; any choice it makes is saved
/// on this computer (013 FR-003).
pub fn set_state(machine: &MachineSettings, wanted: LogState) -> TroubleshootingLog {
    if let Some(choice) = logging::set_state(wanted) {
        machine.set_troubleshooting_log(choice);
    }
    logging::status()
}

/// Shows the file `path` names, if it is there, in the OS's file manager.
pub fn show(opener: &dyn Opener, path: Option<&Path>) -> Result<(), CommandError> {
    let Some(path) = path.filter(|path| path.exists()) else {
        return Err(CommandError::not_found("There is no log file yet."));
    };
    opener.reveal(path).map_err(|failure| {
        // The kind only: a portal's or shell's message can name the file.
        let kind = match failure {
            OpenFailure::NoApp => "nothing can show it",
            OpenFailure::Other(_) => "the OS refused",
        };
        log::warn!("could not show the troubleshooting log: {kind}");
        CommandError::new("INTERNAL_ERROR", "The log file couldn't be shown.")
    })
}

#[tauri::command]
pub async fn get_troubleshooting_log() -> Result<TroubleshootingLog, CommandError> {
    Ok(logging::status())
}

#[tauri::command]
pub async fn set_troubleshooting_log(
    state: LogState,
    machine: State<'_, MachineSettings>,
) -> Result<TroubleshootingLog, CommandError> {
    Ok(set_state(&machine, state))
}

/// The OS's file manager may take a moment, so not on an async thread.
#[tauri::command]
pub async fn show_troubleshooting_log(app: AppHandle) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let opener = app.state::<Arc<dyn Opener>>();
        show(&**opener, logging::current_path().as_deref())
    })
    .await
    .map_err(|_| CommandError::new("INTERNAL_ERROR", "The log file couldn't be shown."))?
}
