//! The page's color mode and layout, kept in `machine.json` because the main
//! web view keeps no storage (code scanning alert `plaintext-lifetime/cwe-922`).
//! Unscoped: it touches no database. The page reads it at start from
//! `window.__HOPLODEX_APPEARANCE__`, which `main.rs` sets.

use tauri::State;

use crate::commands::error::CommandError;
use crate::models::appearance::Appearance;
use crate::services::machine_settings::MachineSettings;

#[tauri::command]
pub async fn set_appearance(
    appearance: Appearance,
    machine: State<'_, MachineSettings>,
) -> Result<(), CommandError> {
    machine.set_appearance(appearance);
    Ok(())
}
