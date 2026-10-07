//! The entry commands of specs/004-cartridges-action-types:
//! `suggest_entries`, `settle_entry` and `list_action_types`
//! (contracts/tauri-commands.md), and of specs/005-regulated-item-types:
//! `list_firearm_types` and `list_registration_classes`.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::models::accessory_kind::{AccessoryKind, AccessoryKindsOutput};
use crate::models::action_type::{ActionType, ActionTypesOutput};
use crate::models::firearm_type::{FirearmTypeInfo, FirearmTypesOutput};
use crate::models::registration::{RegistrationClass, RegistrationClassesOutput};
use crate::services::cartridges::{DerivedCaliber, derive_caliber};
use crate::services::entry_text::EntryField;
pub use crate::services::suggestions::Suggestion;
use crate::services::suggestions::{self, ChangedBy, FieldVocabulary};
use crate::session::Session;

/// Input for `suggest_entries`: what is typed so far in one of the four
/// fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestEntriesInput {
    pub field: EntryField,
    /// May be empty; over 100 characters matches nothing.
    pub text: String,
    /// Model only: the make on the form; only its models are offered (FR-012).
    #[serde(default)]
    pub make: Option<String>,
}

/// At most 20 suggestions, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestEntriesOutput {
    pub suggestions: Vec<Suggestion>,
}

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

    /// The ranked suggestions for one field, from the values on record and
    /// the catalog, computed now and kept nowhere (FR-011; research.md
    /// §4–§5).
    pub fn suggest_entries(
        conn: &Connection,
        input: &SuggestEntriesInput,
    ) -> Result<SuggestEntriesOutput, CommandError> {
        let suggestions =
            suggestions::suggest(conn, input.field, &input.text, input.make.as_deref())
                .map_err(CommandError::from_db)?;
        Ok(SuggestEntriesOutput { suggestions })
    }

    /// research.md §6–§7. A value that breaks FR-015 comes back trimmed and
    /// unchanged: the form reports the rule, and saving enforces it.
    pub fn settle_entry(
        conn: &Connection,
        input: &SettleEntryInput,
    ) -> Result<SettleEntryOutput, CommandError> {
        let vocabulary = FieldVocabulary::load(conn, input.field).map_err(CommandError::from_db)?;
        let (value, changed_by) = suggestions::snap(&vocabulary, &input.text);
        let derived_caliber = match input.field {
            EntryField::Cartridge => match derive_caliber(&value) {
                Some(derived) => {
                    let calibers = FieldVocabulary::load(conn, EntryField::Caliber)
                        .map_err(CommandError::from_db)?;
                    let (caliber, _) = suggestions::snap(&calibers, &derived.caliber);
                    Some(DerivedCaliber { caliber, ..derived })
                }
                None => None,
            },
            _ => None,
        };
        Ok(SettleEntryOutput { value, changed_by, derived_caliber })
    }

    /// The fixed action list and which actions each firearm type allows
    /// (FR-017, FR-018). A type with no mapped actions is left out, which
    /// means every action is allowed.
    pub fn list_action_types(conn: &Connection) -> Result<ActionTypesOutput, CommandError> {
        let mut stmt = conn
            .prepare("SELECT id, name FROM action_types ORDER BY sort_order")
            .map_err(CommandError::from_db)?;
        let actions = stmt
            .query_map([], |row| Ok(ActionType { id: row.get(0)?, name: row.get(1)? }))
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;

        let mut stmt = conn
            .prepare(
                "SELECT m.firearm_type_id, m.action_type_id
                 FROM firearm_type_actions m
                 JOIN action_types a ON a.id = m.action_type_id
                 ORDER BY m.firearm_type_id, a.sort_order",
            )
            .map_err(CommandError::from_db)?;
        let mut allowed_by_firearm_type: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .map_err(CommandError::from_db)?;
        for row in rows {
            let (firearm_type_id, action_type_id) = row.map_err(CommandError::from_db)?;
            allowed_by_firearm_type.entry(firearm_type_id).or_default().push(action_type_id);
        }
        Ok(ActionTypesOutput { actions, allowed_by_firearm_type })
    }

    /// The fixed firearm types, in list order (`sort_order`), with the fields each omits (FR-001,
    /// FR-003; research.md §3): the frontend reads the list instead of
    /// copying it.
    pub fn list_firearm_types(conn: &Connection) -> Result<FirearmTypesOutput, CommandError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, generic_thumbnail_key, action_type_applies,
                        barrel_length_applies, capacity_applies, caliber_from_cartridge
                 FROM firearm_types ORDER BY sort_order",
            )
            .map_err(CommandError::from_db)?;
        let types = stmt
            .query_map([], |row| {
                Ok(FirearmTypeInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    generic_thumbnail_key: row.get(2)?,
                    action_type_applies: row.get(3)?,
                    barrel_length_applies: row.get(4)?,
                    capacity_applies: row.get(5)?,
                    caliber_from_cartridge: row.get(6)?,
                })
            })
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;
        Ok(FirearmTypesOutput { types })
    }

    /// The fixed registration classifications in list order, including any
    /// no longer offered (FR-007; research.md §5): a record that holds one
    /// still shows it.
    pub fn list_registration_classes(
        conn: &Connection,
    ) -> Result<RegistrationClassesOutput, CommandError> {
        let mut stmt = conn
            .prepare("SELECT id, name, offered FROM registration_classes ORDER BY sort_order")
            .map_err(CommandError::from_db)?;
        let classes = stmt
            .query_map([], |row| {
                Ok(RegistrationClass { id: row.get(0)?, name: row.get(1)?, offered: row.get(2)? })
            })
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;
        Ok(RegistrationClassesOutput { classes })
    }

    /// The fixed accessory kinds in list order (`sort_order`), offered or
    /// not (FR-002): a record that holds a kind no longer offered still shows
    /// it, and the frontend reads the list instead of copying it.
    pub fn list_accessory_kinds(conn: &Connection) -> Result<AccessoryKindsOutput, CommandError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, generic_thumbnail_key, sort_order, offered
                 FROM accessory_kinds ORDER BY sort_order",
            )
            .map_err(CommandError::from_db)?;
        let kinds = stmt
            .query_map([], |row| {
                Ok(AccessoryKind {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    generic_thumbnail_key: row.get(2)?,
                    sort_order: row.get(3)?,
                    offered: row.get(4)?,
                })
            })
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;
        Ok(AccessoryKindsOutput { kinds })
    }
}

#[tauri::command]
pub async fn suggest_entries(
    input: SuggestEntriesInput,
    session: State<'_, Session>,
) -> Result<SuggestEntriesOutput, CommandError> {
    session.read(|conn| ops::suggest_entries(conn, &input))
}

#[tauri::command]
pub async fn settle_entry(
    input: SettleEntryInput,
    session: State<'_, Session>,
) -> Result<SettleEntryOutput, CommandError> {
    session.read(|conn| ops::settle_entry(conn, &input))
}

#[tauri::command]
pub async fn list_action_types(
    session: State<'_, Session>,
) -> Result<ActionTypesOutput, CommandError> {
    session.read(ops::list_action_types)
}

#[tauri::command]
pub async fn list_firearm_types(
    session: State<'_, Session>,
) -> Result<FirearmTypesOutput, CommandError> {
    session.read(ops::list_firearm_types)
}

#[tauri::command]
pub async fn list_registration_classes(
    session: State<'_, Session>,
) -> Result<RegistrationClassesOutput, CommandError> {
    session.read(ops::list_registration_classes)
}

#[tauri::command]
pub async fn list_accessory_kinds(
    session: State<'_, Session>,
) -> Result<AccessoryKindsOutput, CommandError> {
    session.read(ops::list_accessory_kinds)
}
