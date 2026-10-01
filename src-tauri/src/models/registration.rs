//! specs/005-regulated-item-types FR-007: the fixed registration
//! classifications, as `list_registration_classes` returns them
//! (contracts/tauri-commands.md; research.md §5).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationClass {
    pub id: i64,
    pub name: String,
    /// `false` = no longer offered for new choices (FR-007). A record that
    /// holds it keeps it.
    pub offered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationClassesOutput {
    /// Every classification, in list order (`sort_order`).
    pub classes: Vec<RegistrationClass>,
}
