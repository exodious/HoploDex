//! Value-summary computation (FR-015): collection total plus a per-policy
//! and unassigned-group breakdown, always recomputed fresh from the
//! current active firearms — there is no cached total to go stale.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

use crate::commands::CommandError;
use crate::models::firearm::CoverageKind;
use crate::services::insurance_status::load_policy_aggregates;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndividualCoverage {
    pub firearm_id: i64,
    pub estimated_value: i64,
    pub scheduled_amount: i64,
    pub under_insured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySummary {
    pub policy_id: i64,
    pub policy_name: String,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
    pub blanket_total: i64,
    pub blanket_limit: i64,
    pub blanket_under_insured: bool,
    pub individually_scheduled: Vec<IndividualCoverage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnassignedFirearm {
    pub firearm_id: i64,
    pub estimated_value: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSummary {
    pub collection_total: i64,
    pub by_policy: Vec<PolicySummary>,
    pub unassigned: Vec<UnassignedFirearm>,
}

/// Input is always "current active, non-disposed firearms" (FR-025) — the
/// frontend re-invokes this after every mutating command rather than
/// maintaining its own running total (contracts/tauri-commands.md).
pub fn get_value_summary(conn: &Connection) -> Result<ValueSummary, CommandError> {
    let policies = load_policy_aggregates(conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, estimated_value, insurance_policy_id, coverage_kind, scheduled_coverage_amount
             FROM firearms WHERE status = 'active'",
        )
        .map_err(CommandError::from_db)?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<CoverageKind>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(CommandError::from_db)?;

    let mut collection_total = 0i64;
    let mut by_policy: HashMap<i64, PolicySummary> = HashMap::new();
    let mut unassigned = Vec::new();

    for row in rows {
        let (
            firearm_id,
            estimated_value,
            insurance_policy_id,
            coverage_kind,
            scheduled_coverage_amount,
        ) = row.map_err(CommandError::from_db)?;
        collection_total += estimated_value.unwrap_or(0);

        let Some(policy_id) = insurance_policy_id else {
            unassigned.push(UnassignedFirearm {
                firearm_id,
                estimated_value: estimated_value.unwrap_or(0),
            });
            continue;
        };

        let aggregate = policies.get(&policy_id);
        let entry = by_policy.entry(policy_id).or_insert_with(|| PolicySummary {
            policy_id,
            policy_name: aggregate.map(|a| a.name.clone()).unwrap_or_default(),
            is_expired: aggregate.map(|a| a.is_expired).unwrap_or(false),
            is_expiring_soon: aggregate.map(|a| a.is_expiring_soon).unwrap_or(false),
            blanket_total: aggregate.map(|a| a.blanket_total).unwrap_or(0),
            blanket_limit: aggregate.map(|a| a.blanket_limit).unwrap_or(0),
            blanket_under_insured: aggregate
                .map(|a| a.is_expired || a.blanket_total > a.blanket_limit)
                .unwrap_or(false),
            individually_scheduled: Vec::new(),
        });

        if coverage_kind == Some(CoverageKind::IndividuallyScheduled) {
            let value = estimated_value.unwrap_or(0);
            let scheduled = scheduled_coverage_amount.unwrap_or(0);
            let under_insured = entry.is_expired || scheduled < value;
            entry.individually_scheduled.push(IndividualCoverage {
                firearm_id,
                estimated_value: value,
                scheduled_amount: scheduled,
                under_insured,
            });
        }
    }

    let mut by_policy: Vec<PolicySummary> = by_policy.into_values().collect();
    by_policy.sort_by_key(|p| p.policy_id);

    Ok(ValueSummary { collection_total, by_policy, unassigned })
}
