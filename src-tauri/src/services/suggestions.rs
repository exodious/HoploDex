//! Suggestion ranking and snapping for make, model, cartridge and caliber
//! (specs/004-cartridges-action-types research.md §4 and §6).

use serde::Serialize;

/// Why a settled value differs from what was typed (research.md §6): it was
/// snapped to the catalog's spelling or to one already on record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangedBy {
    Catalog,
    Record,
}
