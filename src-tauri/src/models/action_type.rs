//! specs/004-cartridges-action-types FR-017/FR-018: the fixed action list and
//! its mapping to firearm types, as `list_action_types` returns them
//! (contracts/tauri-commands.md; research.md §10).

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionType {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionTypesOutput {
    /// Every action, in list order (`sort_order`).
    pub actions: Vec<ActionType>,
    /// Firearm type id → the ids of the actions it allows, in list order. A
    /// type that is absent allows every action (FR-017).
    pub allowed_by_firearm_type: BTreeMap<i64, Vec<i64>>,
}
