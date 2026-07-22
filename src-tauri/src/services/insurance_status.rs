//! Pure business logic behind FR-016/017/024/028's under/uninsured and
//! policy-expiry rules (research.md §9: no direct SQL beyond the one
//! aggregate load below; the actual warning decision is a plain function
//! over already-loaded data, unit/integration-testable without the IPC
//! layer).

use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::commands::CommandError;
use crate::models::firearm::CoverageKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsuranceWarning {
    None,
    Uninsured,
    UnderInsured,
}

/// Per-policy figures needed to evaluate every firearm assigned to it,
/// loaded once per `list_firearms`/`get_value_summary` call rather than
/// per-firearm, to stay within the 500ms/10k-record budget (Principle IV).
#[derive(Debug, Clone)]
pub struct PolicyAggregate {
    pub id: i64,
    pub name: String,
    pub blanket_limit: i64,
    pub blanket_total: i64,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
}

/// Loads every InsurancePolicy along with the summed estimated value of its
/// active blanket-covered firearms, and each policy's expiry status as of
/// today.
pub fn load_policy_aggregates(
    conn: &Connection,
) -> Result<HashMap<i64, PolicyAggregate>, CommandError> {
    let today = chrono::Local::now().date_naive();

    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.blanket_coverage_limit, p.effective_end_date,
                    COALESCE(SUM(
                        CASE WHEN f.coverage_kind = 'blanket' AND f.status = 'active'
                             THEN f.estimated_value ELSE 0 END
                    ), 0) AS blanket_total
             FROM insurance_policies p
             LEFT JOIN firearms f ON f.insurance_policy_id = p.id
             GROUP BY p.id",
        )
        .map_err(CommandError::from_db)?;

    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let blanket_limit: i64 = row.get(2)?;
            let end_date: String = row.get(3)?;
            let blanket_total: i64 = row.get(4)?;
            Ok((id, name, blanket_limit, end_date, blanket_total))
        })
        .map_err(CommandError::from_db)?;

    let mut aggregates = HashMap::new();
    for row in rows {
        let (id, name, blanket_limit, end_date, blanket_total) =
            row.map_err(CommandError::from_db)?;
        let (is_expired, is_expiring_soon) = expiry_status(&end_date, today);
        aggregates.insert(
            id,
            PolicyAggregate {
                id,
                name,
                blanket_limit,
                blanket_total,
                is_expired,
                is_expiring_soon,
            },
        );
    }
    Ok(aggregates)
}

/// `is_expired`: end date is before today. `is_expiring_soon`: within 30
/// days of today and not yet expired (data-model.md's "Derived status").
fn expiry_status(effective_end_date: &str, today: NaiveDate) -> (bool, bool) {
    match NaiveDate::parse_from_str(effective_end_date, "%Y-%m-%d") {
        Ok(end) => {
            let is_expired = end < today;
            let is_expiring_soon = !is_expired && (end - today).num_days() <= 30;
            (is_expired, is_expiring_soon)
        }
        Err(_) => (false, false),
    }
}

/// The single source of truth for a firearm's insurance-warning flag,
/// shared by `list_firearms` (US2) and `get_value_summary` (US3) so the
/// browse view and the value summary never disagree.
pub fn firearm_warning(
    estimated_value: Option<i64>,
    insurance_policy_id: Option<i64>,
    coverage_kind: Option<CoverageKind>,
    scheduled_coverage_amount: Option<i64>,
    policies: &HashMap<i64, PolicyAggregate>,
) -> InsuranceWarning {
    // Edge Case: no value set is not itself a warning condition.
    let value = match estimated_value {
        Some(v) if v > 0 => v,
        _ => return InsuranceWarning::None,
    };

    let Some(policy_id) = insurance_policy_id else {
        return InsuranceWarning::Uninsured;
    };
    let Some(policy) = policies.get(&policy_id) else {
        return InsuranceWarning::Uninsured;
    };
    // FR-024/028, US3 Acceptance Scenario 8: an expired policy overrides
    // otherwise-sufficient coverage — the firearm is treated as uninsured.
    if policy.is_expired {
        return InsuranceWarning::Uninsured;
    }

    match coverage_kind {
        Some(CoverageKind::IndividuallyScheduled) => {
            if scheduled_coverage_amount.unwrap_or(0) < value {
                InsuranceWarning::UnderInsured
            } else {
                InsuranceWarning::None
            }
        }
        Some(CoverageKind::Blanket) => {
            if policy.blanket_total > policy.blanket_limit {
                InsuranceWarning::UnderInsured
            } else {
                InsuranceWarning::None
            }
        }
        None => InsuranceWarning::Uninsured,
    }
}
