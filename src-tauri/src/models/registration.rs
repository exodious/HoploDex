//! 005 FR-007: the fixed registration
//! classifications, as `list_registration_classes` returns them
//! (005 contracts/tauri-commands.md; 005 research.md §5).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationClass {
    pub id: i64,
    pub name: String,
    /// `false` = no longer offered for new choices (005 FR-007). A record that
    /// holds it keeps it.
    pub offered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationClassesOutput {
    /// Every classification, in list order (`sort_order`).
    pub classes: Vec<RegistrationClass>,
}
