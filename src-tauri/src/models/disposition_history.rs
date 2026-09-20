use rusqlite::Row;
use serde::Serialize;

use crate::models::firearm::DispositionType;

/// A retained past disposition of a firearm that was later restored to
/// active status (FR-033). Read-only: written only by `reverse_disposition`
/// with `keep`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispositionHistoryEntry {
    pub id: i64,
    pub firearm_id: i64,
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
            firearm_id: row.get("firearm_id")?,
            disposition_type: row.get("disposition_type")?,
            disposition_recipient: row.get("disposition_recipient")?,
            disposition_date: row.get("disposition_date")?,
            disposition_price: row.get("disposition_price")?,
            reversed_at: row.get("reversed_at")?,
        })
    }
}
