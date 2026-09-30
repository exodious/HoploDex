//! specs/005-regulated-item-types FR-001/FR-003: the fixed firearm types and
//! the fields each one omits, as `list_firearm_types` returns them
//! (contracts/tauri-commands.md; research.md §3).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmTypeInfo {
    pub id: i64,
    pub name: String,
    pub generic_thumbnail_key: String,
    /// `false` = the type has no action: the form doesn't offer it and no
    /// firearm of the type may hold one (FR-003).
    pub action_type_applies: bool,
    pub barrel_length_applies: bool,
    pub capacity_applies: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmTypesOutput {
    /// Every type, in id order.
    pub types: Vec<FirearmTypeInfo>,
}
