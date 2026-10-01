use rusqlite::Row;
use serde::Serialize;

use crate::models::firearm::DispositionType;
use crate::models::record::RecordRef;

/// A retained past disposition of a firearm or accessory that was later
/// restored to active status (FR-033; specs/006-accessory-links FR-006).
/// Read-only: written only by `reverse_disposition` and
/// `reverse_accessory_disposition` with `keep`. The record it belongs to is
/// its `owner`, read from the `firearm_id` / `accessory_id` column pair
/// (research.md §3).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispositionHistoryEntry {
    pub id: i64,
    pub owner: RecordRef,
    pub disposition_type: DispositionType,
    pub disposition_recipient: String,
    pub disposition_date: String,
    pub disposition_price: Option<i64>,
    pub reversed_at: String,
}

impl DispositionHistoryEntry {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            owner: RecordRef::from_owner_row(row)?,
            disposition_type: row.get("disposition_type")?,
            disposition_recipient: row.get("disposition_recipient")?,
            disposition_date: row.get("disposition_date")?,
            disposition_price: row.get("disposition_price")?,
            reversed_at: row.get("reversed_at")?,
        })
    }
}
