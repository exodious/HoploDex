//! Value-summary computation (FR-015; 006 FR-008/FR-009): the collection
//! total with its firearms and accessories subtotals, the blanket policy in
//! force against its limit, each policy's individually scheduled records, and
//! the records nothing covers — always recomputed fresh from the current
//! active firearms and accessories, so there is no cached total to go stale.

use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::commands::CommandError;
use crate::models::record::RecordRef;
use crate::services::insurance_status::{InsuranceContext, load_context_as_of};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndividualCoverage {
    pub record: RecordRef,
    pub estimated_value: i64,
    pub scheduled_amount: i64,
    pub under_insured: bool,
}

/// A policy that has records scheduled under it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySummary {
    pub policy_id: i64,
    pub policy_name: String,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
    pub individually_scheduled: Vec<IndividualCoverage>,
}

/// The blanket policy in force today, against the combined value of every
/// active record (firearm or accessory) that isn't individually scheduled.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlanketSummary {
    pub policy_id: i64,
    pub policy_name: String,
    pub limit: i64,
    pub total: i64,
    pub firearm_count: i64,
    pub accessory_count: i64,
    pub under_insured: bool,
}

/// An unscheduled record with no blanket policy in force to cover it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninsuredRecord {
    pub record: RecordRef,
    pub estimated_value: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSummary {
    pub collection_total: i64,
    pub firearms_total: i64,
    pub accessories_total: i64,
    pub blanket: Option<BlanketSummary>,
    pub by_policy: Vec<PolicySummary>,
    pub uninsured: Vec<UninsuredRecord>,
}

/// Input is always "current active, non-disposed firearms and accessories" (FR-025) — the
/// frontend re-invokes this after every mutating command rather than
/// maintaining its own running total (contracts/tauri-commands.md).
pub fn get_value_summary(conn: &Connection) -> Result<ValueSummary, CommandError> {
    get_value_summary_as_of(conn, chrono::Local::now().date_naive())
}

/// [`get_value_summary`] as of an explicit "today".
pub fn get_value_summary_as_of(
    conn: &Connection,
    today: NaiveDate,
) -> Result<ValueSummary, CommandError> {
    let ctx = load_context_as_of(conn, today)?;

    // The two tables, one pass each, in one shape.
    let mut stmt = conn
        .prepare(
            "SELECT 0, id, estimated_value, insurance_policy_id, scheduled_coverage_amount
             FROM firearms WHERE status = 'active'
             UNION ALL
             SELECT 1, id, estimated_value, insurance_policy_id, scheduled_coverage_amount
             FROM accessories WHERE status = 'active'",
        )
        .map_err(CommandError::from_db)?;
    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(1)?;
            let record = match row.get::<_, i64>(0)? {
                0 => RecordRef::Firearm(id),
                _ => RecordRef::Accessory(id),
            };
            Ok((
                record,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(CommandError::from_db)?;

    let (mut firearms_total, mut accessories_total) = (0i64, 0i64);
    let mut by_policy: HashMap<i64, PolicySummary> = HashMap::new();
    let mut uninsured = Vec::new();

    for row in rows {
        let (record, estimated_value, policy_id, scheduled_amount) =
            row.map_err(CommandError::from_db)?;
        let value = estimated_value.unwrap_or(0);
        match record {
            RecordRef::Firearm(_) => firearms_total += value,
            RecordRef::Accessory(_) => accessories_total += value,
        }

        match policy_id {
            Some(policy_id) => {
                let entry =
                    by_policy.entry(policy_id).or_insert_with(|| policy_summary(policy_id, &ctx));
                let scheduled = scheduled_amount.unwrap_or(0);
                entry.individually_scheduled.push(IndividualCoverage {
                    record,
                    estimated_value: value,
                    scheduled_amount: scheduled,
                    under_insured: entry.is_expired || scheduled < value,
                });
            }
            // A record with no value is not listed: no value set is not a
            // warning condition (001 Edge Cases), as for the record's own warning.
            None if ctx.blanket.is_none() && value > 0 => {
                uninsured.push(UninsuredRecord { record, estimated_value: value });
            }
            None => {}
        }
    }

    let mut by_policy: Vec<PolicySummary> = by_policy.into_values().collect();
    by_policy.sort_by(|a, b| a.policy_name.cmp(&b.policy_name).then(a.policy_id.cmp(&b.policy_id)));

    let blanket = ctx.blanket.map(|b| BlanketSummary {
        under_insured: b.total > b.limit,
        policy_id: b.policy_id,
        policy_name: b.policy_name,
        limit: b.limit,
        total: b.total,
        firearm_count: b.firearm_count,
        accessory_count: b.accessory_count,
    });

    Ok(ValueSummary {
        collection_total: firearms_total + accessories_total,
        firearms_total,
        accessories_total,
        blanket,
        by_policy,
        uninsured,
    })
}

fn policy_summary(policy_id: i64, ctx: &InsuranceContext) -> PolicySummary {
    let status = ctx.policies.get(&policy_id);
    PolicySummary {
        policy_id,
        policy_name: status.map(|s| s.name.clone()).unwrap_or_default(),
        is_expired: status.is_some_and(|s| s.is_expired),
        is_expiring_soon: status.is_some_and(|s| s.is_expiring_soon),
        individually_scheduled: Vec::new(),
    }
}
