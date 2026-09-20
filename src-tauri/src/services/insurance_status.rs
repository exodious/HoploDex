//! Pure business logic behind FR-016/017/024/028/036's under/uninsured and
//! policy-expiry rules. Coverage is implicit: a firearm is either
//! individually scheduled under a policy (with its own amount) or covered by
//! the blanket policy in force on the day, which is computed from policy dates
//! and never stored per firearm (research.md §9: no direct SQL beyond the
//! loads below; the warning decision is a plain function over the loaded
//! [`InsuranceContext`], testable without the IPC layer).

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::commands::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsuranceWarning {
    None,
    Uninsured,
    UnderInsured,
}

/// One policy's derived status as of a day (data-model.md, "Derived
/// status"). `is_*` are facts about the dates; the `*_warning` flags are what
/// the user is warned about, which FR-028 suppresses for a blanket policy
/// that has been renewed or replaced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyStatus {
    #[serde(skip)]
    pub id: i64,
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub is_blanket: bool,
    pub is_in_force: bool,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
    pub expiring_warning: bool,
    pub expired_warning: bool,
}

/// The blanket policy in force on the day, with the total value of the
/// active firearms that fall under it (every active firearm that is not
/// individually scheduled).
#[derive(Debug, Clone)]
pub struct BlanketInForce {
    pub policy_id: i64,
    pub policy_name: String,
    pub limit: i64,
    pub total: i64,
    pub firearm_count: i64,
}

/// Everything needed to evaluate any firearm's coverage, loaded once per
/// `list_firearms`/`get_value_summary` call rather than per firearm, to stay
/// within the 500ms/10k-record budget (Principle IV).
#[derive(Debug, Clone)]
pub struct InsuranceContext {
    pub policies: HashMap<i64, PolicyStatus>,
    pub blanket: Option<BlanketInForce>,
}

struct PolicyDates {
    id: i64,
    name: String,
    blanket_limit: Option<i64>,
    start: NaiveDate,
    end: NaiveDate,
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

pub fn load_context(conn: &Connection) -> Result<InsuranceContext, CommandError> {
    load_context_as_of(conn, chrono::Local::now().date_naive())
}

/// [`load_context`] for an explicit "today", so boundary days and expiry
/// can be exercised exactly.
pub fn load_context_as_of(
    conn: &Connection,
    today: NaiveDate,
) -> Result<InsuranceContext, CommandError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, blanket_coverage_limit, effective_start_date, effective_end_date
             FROM insurance_policies",
        )
        .map_err(CommandError::from_db)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(CommandError::from_db)?;

    let mut policies = Vec::new();
    for row in rows {
        let (id, name, blanket_limit, start, end) = row.map_err(CommandError::from_db)?;
        // Dates are validated on write; a row that somehow isn't parseable
        // can't be placed in time, so it takes no part in coverage.
        if let (Some(start), Some(end)) = (parse_date(&start), parse_date(&end)) {
            policies.push(PolicyDates { id, name, blanket_limit, start, end });
        }
    }

    let statuses: HashMap<i64, PolicyStatus> =
        policies.iter().map(|p| (p.id, status_of(p, &policies, today))).collect();

    // On a shared boundary day the later-starting blanket policy is in force.
    let in_force = policies
        .iter()
        .filter(|p| p.blanket_limit.is_some() && p.start <= today && today <= p.end)
        .max_by_key(|p| (p.start, p.id));

    let blanket = match in_force {
        None => None,
        Some(policy) => {
            let (total, firearm_count) = conn
                .query_row(
                    "SELECT COALESCE(SUM(COALESCE(estimated_value, 0)), 0), COUNT(*)
                     FROM firearms WHERE status = 'active' AND insurance_policy_id IS NULL",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .map_err(CommandError::from_db)?;
            Some(BlanketInForce {
                policy_id: policy.id,
                policy_name: policy.name.clone(),
                limit: policy.blanket_limit.unwrap_or(0),
                total,
                firearm_count,
            })
        }
    };

    Ok(InsuranceContext { policies: statuses, blanket })
}

fn status_of(policy: &PolicyDates, all: &[PolicyDates], today: NaiveDate) -> PolicyStatus {
    let is_in_force = policy.start <= today && today <= policy.end;
    let is_expired = policy.end < today;
    let is_expiring_soon = !is_expired && (policy.end - today).num_days() <= 30;

    // FR-028: a blanket policy's lapse doesn't warrant a warning when another
    // blanket policy takes over. An expiring warning is suppressed while a
    // successor starts no later than the day after this one ends (no gap); an
    // expired one once any later-starting blanket policy exists (history).
    let is_blanket = policy.blanket_limit.is_some();
    let successors = || {
        all.iter().filter(move |other| {
            is_blanket
                && other.blanket_limit.is_some()
                && other.id != policy.id
                && other.start > policy.start
        })
    };
    let seamless_successor = successors().any(|s| s.start <= policy.end + Duration::days(1));
    let any_successor = successors().next().is_some();

    PolicyStatus {
        id: policy.id,
        name: policy.name.clone(),
        is_blanket,
        is_in_force,
        is_expired,
        is_expiring_soon,
        expiring_warning: is_expiring_soon && !seamless_successor,
        expired_warning: is_expired && !any_successor,
    }
}

/// The single source of truth for a firearm's insurance-warning flag,
/// shared by `list_firearms` (US2) and `get_value_summary` (US3) so the
/// browse view and the value summary never disagree.
pub fn firearm_warning(
    estimated_value: Option<i64>,
    insurance_policy_id: Option<i64>,
    scheduled_coverage_amount: Option<i64>,
    ctx: &InsuranceContext,
) -> InsuranceWarning {
    // Edge Case: no value set is not itself a warning condition.
    let value = match estimated_value {
        Some(v) if v > 0 => v,
        _ => return InsuranceWarning::None,
    };

    match insurance_policy_id {
        // Individually scheduled (FR-024): its own amount against its value.
        // An expired policy overrides otherwise-sufficient coverage
        // (US3 Acceptance Scenario 8).
        Some(policy_id) => match ctx.policies.get(&policy_id) {
            None => InsuranceWarning::Uninsured,
            Some(policy) if policy.is_expired => InsuranceWarning::Uninsured,
            Some(_) if scheduled_coverage_amount.unwrap_or(0) < value => {
                InsuranceWarning::UnderInsured
            }
            Some(_) => InsuranceWarning::None,
        },
        // Unscheduled: covered by the blanket policy in force (FR-036), whose
        // shared limit is compared with the combined value of all of them
        // (FR-017); uninsured when none is in force.
        None => match &ctx.blanket {
            None => InsuranceWarning::Uninsured,
            Some(blanket) if blanket.total > blanket.limit => InsuranceWarning::UnderInsured,
            Some(_) => InsuranceWarning::None,
        },
    }
}
