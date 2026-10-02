//! specs/006-accessory-links research.md §15: the fixed accessory kinds, as
//! `list_accessory_kinds` returns them (FR-002).

use serde::Serialize;

/// One row of `accessory_kinds`. A kind with `offered` false is still shown
/// on the records that hold it and still accepted by the commands, but not
/// offered for new choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessoryKind {
    pub id: i64,
    pub name: String,
    pub generic_thumbnail_key: String,
    pub sort_order: i64,
    pub offered: bool,
}

/// `list_accessory_kinds`' output: every kind, offered or not, in
/// `sort_order`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccessoryKindsOutput {
    pub kinds: Vec<AccessoryKind>,
}
