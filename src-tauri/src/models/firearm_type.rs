//! 005 FR-001/FR-003: the fixed firearm types and
//! the fields each one omits, as `list_firearm_types` returns them
//! (005 contracts/tauri-commands.md; 005 research.md §3).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmTypeInfo {
    pub id: i64,
    pub name: String,
    pub generic_thumbnail_key: String,
    /// `false` = the type has no action: the form doesn't offer it and no
    /// firearm of the type may hold one (005 FR-003).
    pub action_type_applies: bool,
    pub barrel_length_applies: bool,
    pub capacity_applies: bool,
    /// `false` = the caliber is never worked out from the cartridge, on the
    /// form or on import: a Suppressor's caliber is its bore (005 FR-002;
    /// 005 research.md §15).
    pub caliber_from_cartridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmTypesOutput {
    /// Every type, in list order (`sort_order`): Other last.
    pub types: Vec<FirearmTypeInfo>,
}
