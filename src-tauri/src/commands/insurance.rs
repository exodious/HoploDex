use rusqlite::{Connection, OptionalExtension, named_params};
use serde::Deserialize;
use tauri::State;

use crate::commands::CommandError;
use crate::commands::firearms;
use crate::models::firearm::Firearm;
use crate::models::insurance_policy::{
    InsurancePolicy, InsurancePolicyInput, validate_insurance_policy_input,
};
use crate::services::insurance_status::{PolicyStatus, load_context};
use crate::services::valuation::{self, ValueSummary};
use crate::session::Session;

/// Input for `assign_firearm_coverage`, per contracts/tauri-commands.md.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignCoverageInput {
    pub policy_id: Option<i64>,
    pub scheduled_coverage_amount: Option<i64>,
}

/// How the firearms scheduled under a policy being deleted are resolved
/// (FR-034): moved to another policy, keeping their scheduled amounts, or
/// left unscheduled — covered by the blanket policy in force, or uninsured.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ScheduledFirearmsAction {
    Move {
        #[serde(rename = "targetPolicyId")]
        target_policy_id: i64,
    },
    Unschedule {
        /// Required to unschedule the firearms of a policy that hasn't
        /// expired: they would lose coverage they are still relying on.
        #[serde(rename = "confirmUnschedule", default)]
        confirm_unschedule: Option<bool>,
    },
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePolicyResult {
    pub deleted: bool,
    pub moved_count: usize,
    pub unscheduled_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledFirearmRef {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtherPolicy {
    pub id: i64,
    pub name: String,
    pub is_expired: bool,
}

/// What deleting a policy would do, for the FR-034 dialog to explain before
/// anything changes.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDeletionImpact {
    pub is_expired: bool,
    pub is_blanket_in_force: bool,
    pub scheduled_firearm_count: i64,
    pub scheduled_firearms: Vec<ScheduledFirearmRef>,
    /// Active unscheduled firearms that lose blanket coverage because this is
    /// the blanket policy in force (0 otherwise).
    pub blanket_firearm_count: i64,
    /// What becomes of firearms left unscheduled: `"blanket"` if another
    /// blanket policy is in force once this one is gone, else `"uninsured"`.
    pub unschedule_outcome: &'static str,
    pub other_policies: Vec<OtherPolicy>,
}

/// A policy as listed: its record plus derived status (in force, expiring,
/// expired, and the FR-028 warnings), so the frontend never re-derives them.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsurancePolicyView {
    #[serde(flatten)]
    pub policy: InsurancePolicy,
    #[serde(flatten)]
    pub status: PolicyStatus,
}

/// Pure, `Connection`-based business logic — mirrors `commands::firearms::ops`
/// (constitution: no mocks, integration tests call these directly against a
/// real temporary SQLCipher database).
pub mod ops {
    use super::*;

    pub fn get_policy(conn: &Connection, id: i64) -> Result<InsurancePolicy, CommandError> {
        conn.query_row(
            "SELECT * FROM insurance_policies WHERE id = :id",
            named_params! { ":id": id },
            InsurancePolicy::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No insurance policy was found with that id."))
    }

    /// Not in contracts/tauri-commands.md's original command list, but
    /// necessary to populate the coverage-assignment policy picker — the
    /// contract only documents commands for mutating/summarizing policies,
    /// not enumerating them.
    pub fn list_policies(conn: &Connection) -> Result<Vec<InsurancePolicy>, CommandError> {
        let mut stmt = conn
            .prepare("SELECT * FROM insurance_policies ORDER BY name")
            .map_err(CommandError::from_db)?;
        let rows = stmt.query_map([], InsurancePolicy::from_row).map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// [`list_policies`] with each policy's derived status as of today.
    pub fn list_policy_views(conn: &Connection) -> Result<Vec<InsurancePolicyView>, CommandError> {
        let context = load_context(conn)?;
        Ok(list_policies(conn)?
            .into_iter()
            .filter_map(|policy| {
                let status = context.policies.get(&policy.id)?.clone();
                Some(InsurancePolicyView { policy, status })
            })
            .collect())
    }

    /// One policy with its derived status as of today.
    pub fn policy_view(
        conn: &Connection,
        policy: InsurancePolicy,
    ) -> Result<InsurancePolicyView, CommandError> {
        let status = load_context(conn)?
            .policies
            .remove(&policy.id)
            .ok_or_else(|| CommandError::new("INTERNAL_ERROR", "An unexpected error occurred."))?;
        Ok(InsurancePolicyView { policy, status })
    }

    /// FR-036: at most one blanket policy in force on any date. Two blanket
    /// policies conflict when their dates overlap by more than a single
    /// shared boundary day (`A.start < B.end AND B.start < A.end`); the
    /// message names the other policy. Schedule-only policies never conflict.
    /// `exclude_id` is the policy being edited, which never clashes with
    /// itself.
    fn check_blanket_overlap(
        conn: &Connection,
        exclude_id: Option<i64>,
        input: &InsurancePolicyInput,
    ) -> Result<(), CommandError> {
        if input.blanket_coverage_limit.is_none() {
            return Ok(());
        }
        let other = conn
            .query_row(
                "SELECT name, effective_start_date, effective_end_date FROM insurance_policies
                 WHERE blanket_coverage_limit IS NOT NULL AND id IS NOT :exclude
                   AND effective_start_date < :end AND :start < effective_end_date
                 ORDER BY effective_start_date LIMIT 1",
                named_params! {
                    ":exclude": exclude_id,
                    ":start": input.effective_start_date,
                    ":end": input.effective_end_date,
                },
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::from_db)?;
        match other {
            None => Ok(()),
            Some((name, start, end)) => {
                let message = format!(
                    "These dates overlap the blanket policy {name} ({start} to {end}). Only one \
                     blanket policy can be in force on any day, though a policy may start on the \
                     day another ends."
                );
                Err(CommandError::validation(
                    message.clone(),
                    [("effectiveStartDate".to_string(), message)].into(),
                ))
            }
        }
    }

    pub fn create_policy(
        conn: &Connection,
        input: &InsurancePolicyInput,
    ) -> Result<InsurancePolicy, CommandError> {
        validate_insurance_policy_input(input)?;
        check_blanket_overlap(conn, None, input)?;
        conn.execute(
            "INSERT INTO insurance_policies (
                name, policy_number, insurance_company, company_contact, agent_name, agent_contact,
                notes, blanket_coverage_limit, effective_start_date, effective_end_date, created_at, updated_at
            ) VALUES (
                :name, :policy_number, :insurance_company, :company_contact, :agent_name, :agent_contact,
                :notes, :blanket_coverage_limit, :effective_start_date, :effective_end_date,
                datetime('now'), datetime('now')
            )",
            named_params! {
                ":name": input.name,
                ":policy_number": input.policy_number,
                ":insurance_company": input.insurance_company,
                ":company_contact": input.company_contact,
                ":agent_name": input.agent_name,
                ":agent_contact": input.agent_contact,
                ":notes": input.normalized_notes(),
                ":blanket_coverage_limit": input.blanket_coverage_limit,
                ":effective_start_date": input.effective_start_date,
                ":effective_end_date": input.effective_end_date,
            },
        )
        .map_err(CommandError::from_db)?;

        get_policy(conn, conn.last_insert_rowid())
    }

    pub fn update_policy(
        conn: &Connection,
        id: i64,
        input: &InsurancePolicyInput,
    ) -> Result<InsurancePolicy, CommandError> {
        validate_insurance_policy_input(input)?;
        check_blanket_overlap(conn, Some(id), input)?;
        let updated = conn
            .execute(
                "UPDATE insurance_policies SET
                    name = :name,
                    policy_number = :policy_number,
                    insurance_company = :insurance_company,
                    company_contact = :company_contact,
                    agent_name = :agent_name,
                    agent_contact = :agent_contact,
                    notes = :notes,
                    blanket_coverage_limit = :blanket_coverage_limit,
                    effective_start_date = :effective_start_date,
                    effective_end_date = :effective_end_date,
                    updated_at = datetime('now')
                WHERE id = :id",
                named_params! {
                    ":id": id,
                    ":name": input.name,
                    ":policy_number": input.policy_number,
                    ":insurance_company": input.insurance_company,
                    ":company_contact": input.company_contact,
                    ":agent_name": input.agent_name,
                    ":agent_contact": input.agent_contact,
                    ":notes": input.normalized_notes(),
                    ":blanket_coverage_limit": input.blanket_coverage_limit,
                    ":effective_start_date": input.effective_start_date,
                    ":effective_end_date": input.effective_end_date,
                },
            )
            .map_err(CommandError::from_db)?;

        if updated == 0 {
            return Err(CommandError::not_found("No insurance policy was found with that id."));
        }
        get_policy(conn, id)
    }

    pub fn get_policy_deletion_impact(
        conn: &Connection,
        id: i64,
    ) -> Result<PolicyDeletionImpact, CommandError> {
        get_policy(conn, id)?;
        let context = load_context(conn)?;
        let status = context
            .policies
            .get(&id)
            .ok_or_else(|| CommandError::new("INTERNAL_ERROR", "An unexpected error occurred."))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, make, model, nickname FROM firearms
                 WHERE insurance_policy_id = :id AND status = 'active'
                 ORDER BY make, model, id",
            )
            .map_err(CommandError::from_db)?;
        let scheduled_firearms = stmt
            .query_map(named_params! { ":id": id }, |row| {
                Ok(ScheduledFirearmRef {
                    id: row.get(0)?,
                    make: row.get(1)?,
                    model: row.get(2)?,
                    nickname: row.get(3)?,
                })
            })
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;

        let in_force = context.blanket.as_ref().filter(|b| b.policy_id == id);
        let another_blanket_in_force =
            context.policies.values().any(|p| p.id != id && p.is_blanket && p.is_in_force);
        let mut other_policies: Vec<OtherPolicy> = context
            .policies
            .values()
            .filter(|p| p.id != id)
            .map(|p| OtherPolicy { id: p.id, name: p.name.clone(), is_expired: p.is_expired })
            .collect();
        other_policies.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));

        Ok(PolicyDeletionImpact {
            is_expired: status.is_expired,
            is_blanket_in_force: in_force.is_some(),
            scheduled_firearm_count: scheduled_firearms.len() as i64,
            scheduled_firearms,
            blanket_firearm_count: in_force.map_or(0, |b| b.firearm_count),
            unschedule_outcome: if another_blanket_in_force { "blanket" } else { "uninsured" },
            other_policies,
        })
    }

    /// Deletes a policy, first resolving the firearms scheduled under it
    /// (FR-034), all in one transaction: an error anywhere leaves everything
    /// as it was. A deletion that leaves active scheduled firearms unresolved
    /// is refused, so no firearm is ever silently un-insured; the foreign key
    /// stays `ON DELETE RESTRICT` as a backstop. Disposed firearms aren't
    /// insured, so they never block a deletion and are cleared along with it.
    pub fn delete_policy(
        conn: &Connection,
        id: i64,
        confirmed: bool,
        resolution: Option<&ScheduledFirearmsAction>,
    ) -> Result<DeletePolicyResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }
        let impact = get_policy_deletion_impact(conn, id)?;
        let active = impact.scheduled_firearm_count;

        let tx = conn.unchecked_transaction().map_err(CommandError::from_db)?;
        let (mut moved_count, mut unscheduled_count) = (0, 0);
        match resolution {
            None if active > 0 => {
                return Err(CommandError::new(
                    "POLICY_HAS_FIREARMS",
                    format!(
                        "This policy still covers {active} firearm(s). Move them to another \
                         policy, or leave them unscheduled, before deleting it."
                    ),
                ));
            }
            Some(ScheduledFirearmsAction::Move { target_policy_id }) => {
                if *target_policy_id == id || get_policy(conn, *target_policy_id).is_err() {
                    return Err(CommandError::new(
                        "VALIDATION_ERROR",
                        "Choose another existing policy to move the firearms to.",
                    ));
                }
                moved_count = conn
                    .execute(
                        "UPDATE firearms SET insurance_policy_id = :target, updated_at = datetime('now')
                         WHERE insurance_policy_id = :id",
                        named_params! { ":target": target_policy_id, ":id": id },
                    )
                    .map_err(CommandError::from_db)?;
            }
            resolution => {
                if let Some(ScheduledFirearmsAction::Unschedule { confirm_unschedule }) = resolution
                    && !impact.is_expired
                    && active > 0
                    && *confirm_unschedule != Some(true)
                {
                    let outcome = if impact.unschedule_outcome == "blanket" {
                        "covered by the blanket policy in force"
                    } else {
                        "uninsured"
                    };
                    return Err(CommandError::new(
                        "VALIDATION_ERROR",
                        format!(
                            "Leaving {active} firearm(s) unscheduled removes their scheduled \
                                 coverage; they would be {outcome}. Confirm to continue."
                        ),
                    ));
                }
                unscheduled_count = conn
                    .execute(
                        "UPDATE firearms SET insurance_policy_id = NULL,
                            scheduled_coverage_amount = NULL, updated_at = datetime('now')
                         WHERE insurance_policy_id = :id",
                        named_params! { ":id": id },
                    )
                    .map_err(CommandError::from_db)?;
            }
        }

        conn.execute("DELETE FROM insurance_policies WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;
        tx.commit().map_err(CommandError::from_db)?;
        Ok(DeletePolicyResult { deleted: true, moved_count, unscheduled_count })
    }

    /// Schedules a firearm under `policy_id` with its own amount, or, with
    /// `None`, unschedules it: it is then covered by the blanket policy in
    /// force (FR-036). There is no per-firearm blanket assignment.
    pub fn assign_firearm_coverage(
        conn: &Connection,
        firearm_id: i64,
        policy_id: Option<i64>,
        scheduled_coverage_amount: Option<i64>,
    ) -> Result<Firearm, CommandError> {
        let current = firearms::ops::get_firearm(conn, firearm_id)?;
        if let Some(policy_id) = policy_id {
            get_policy(conn, policy_id)?;
        }
        let input = crate::models::firearm::FirearmInput {
            insurance_policy_id: policy_id,
            // Unscheduling clears the amount along with the policy.
            scheduled_coverage_amount: policy_id.and(scheduled_coverage_amount),
            ..crate::models::firearm::FirearmInput::from(&current)
        };
        // Never touches an identifying field, so FR-009's warning never
        // applies here (research.md §5); `confirmed_warnings: true` skips it.
        firearms::ops::update_firearm(conn, firearm_id, &input, true)
    }

    pub fn get_value_summary(conn: &Connection) -> Result<ValueSummary, CommandError> {
        valuation::get_value_summary(conn)
    }
}

#[tauri::command]
pub async fn list_insurance_policies(
    session: State<'_, Session>,
) -> Result<Vec<InsurancePolicyView>, CommandError> {
    session.read(ops::list_policy_views)
}

#[tauri::command]
pub async fn create_insurance_policy(
    input: InsurancePolicyInput,
    session: State<'_, Session>,
) -> Result<InsurancePolicyView, CommandError> {
    session.write(|conn| {
        let policy = ops::create_policy(conn, &input)?;
        ops::policy_view(conn, policy)
    })
}

#[tauri::command]
pub async fn update_insurance_policy(
    id: i64,
    input: InsurancePolicyInput,
    session: State<'_, Session>,
) -> Result<InsurancePolicyView, CommandError> {
    session.write(|conn| {
        let policy = ops::update_policy(conn, id, &input)?;
        ops::policy_view(conn, policy)
    })
}

#[tauri::command]
pub async fn get_policy_deletion_impact(
    id: i64,
    session: State<'_, Session>,
) -> Result<PolicyDeletionImpact, CommandError> {
    session.read(|conn| ops::get_policy_deletion_impact(conn, id))
}

#[tauri::command]
pub async fn delete_insurance_policy(
    id: i64,
    confirmed: bool,
    scheduled_firearms: Option<ScheduledFirearmsAction>,
    session: State<'_, Session>,
) -> Result<DeletePolicyResult, CommandError> {
    session.write(|conn| ops::delete_policy(conn, id, confirmed, scheduled_firearms.as_ref()))
}

#[tauri::command]
pub async fn assign_firearm_coverage(
    firearm_id: i64,
    input: AssignCoverageInput,
    session: State<'_, Session>,
) -> Result<Firearm, CommandError> {
    session.write(|conn| {
        ops::assign_firearm_coverage(
            conn,
            firearm_id,
            input.policy_id,
            input.scheduled_coverage_amount,
        )
    })
}

#[tauri::command]
pub async fn get_value_summary(session: State<'_, Session>) -> Result<ValueSummary, CommandError> {
    session.read(ops::get_value_summary)
}
