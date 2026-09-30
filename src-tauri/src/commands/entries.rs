//! The entry commands of specs/004-cartridges-action-types:
//! `suggest_entries`, `settle_entry` and `list_action_types`
//! (contracts/tauri-commands.md).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::services::cartridges::{DerivedCaliber, derive_caliber};
use crate::services::entry_text::EntryField;
use crate::services::suggestions::ChangedBy;
use crate::session::Session;

/// Input for `settle_entry`: a value the user has finished entering (left
/// the field, or picked a suggestion).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleEntryInput {
    pub field: EntryField,
    pub text: String,
}

/// What the value becomes (FR-013) and, for a cartridge, the caliber it
/// derives (FR-003, FR-005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleEntryOutput {
    /// Trimmed; the snapped spelling if any.
    pub value: String,
    /// `None`: kept as typed, apart from trimming.
    pub changed_by: Option<ChangedBy>,
    /// Cartridge only; `None` for a cartridge means no caliber could be
    /// read.
    pub derived_caliber: Option<DerivedCaliber>,
}

/// Pure, `Connection`-based logic, called by the commands below and by the
/// integration tests (constitution: no mocks).
pub mod ops {
    use super::*;

    /// research.md §6–§7. A value that breaks FR-015 comes back trimmed and
    /// unchanged: the form reports the rule, and saving enforces it.
    pub fn settle_entry(
        _conn: &Connection,
        input: &SettleEntryInput,
    ) -> Result<SettleEntryOutput, CommandError> {
        let value = input.text.trim().to_owned();
        let derived_caliber = match input.field {
            EntryField::Cartridge => derive_caliber(&value),
            _ => None,
        };
        Ok(SettleEntryOutput { value, changed_by: None, derived_caliber })
    }
}

#[tauri::command]
pub async fn settle_entry(
    input: SettleEntryInput,
    session: State<'_, Session>,
) -> Result<SettleEntryOutput, CommandError> {
    session.read(|conn| ops::settle_entry(conn, &input))
}
