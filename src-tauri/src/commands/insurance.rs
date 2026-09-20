use rusqlite::{named_params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::commands::firearms::{self, DeleteResult};
use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::firearm::Firearm;
use crate::models::insurance_policy::{
    validate_insurance_policy_input, InsurancePolicy, InsurancePolicyInput,
};
use crate::services::insurance_status::{load_context, PolicyStatus};
use crate::services::valuation::{self, ValueSummary};

/// Input for `assign_firearm_coverage`, per contracts/tauri-commands.md.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignCoverageInput {
    pub policy_id: Option<i64>,
    pub scheduled_coverage_amount: Option<i64>,
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
                blanket_coverage_limit, effective_start_date, effective_end_date, created_at, updated_at
            ) VALUES (
                :name, :policy_number, :insurance_company, :company_contact, :agent_name, :agent_contact,
                :blanket_coverage_limit, :effective_start_date, :effective_end_date,
                datetime('now'), datetime('now')
            )",
            named_params! {
                ":name": input.name,
                ":policy_number": input.policy_number,
                ":insurance_company": input.insurance_company,
                ":company_contact": input.company_contact,
                ":agent_name": input.agent_name,
                ":agent_contact": input.agent_contact,
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

    pub fn delete_policy(
        conn: &Connection,
        id: i64,
        confirmed: bool,
    ) -> Result<DeleteResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }

        let assigned_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM firearms WHERE insurance_policy_id = :id",
                named_params! { ":id": id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if assigned_count > 0 {
            return Err(CommandError::new(
                "POLICY_HAS_FIREARMS",
                format!(
                    "This policy still covers {assigned_count} firearm(s). Reassign or unassign them before deleting it."
                ),
            ));
        }

        let deleted = conn
            .execute("DELETE FROM insurance_policies WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;
        if deleted == 0 {
            return Err(CommandError::not_found("No insurance policy was found with that id."));
        }
        Ok(DeleteResult { deleted: true })
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
        firearms::ops::update_firearm(conn, firearm_id, &input)
    }

    pub fn get_value_summary(conn: &Connection) -> Result<ValueSummary, CommandError> {
        valuation::get_value_summary(conn)
    }
}

#[tauri::command]
pub async fn list_insurance_policies(
    state: State<'_, DbHandle>,
) -> Result<Vec<InsurancePolicyView>, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::list_policy_views(&conn)
}

#[tauri::command]
pub async fn create_insurance_policy(
    input: InsurancePolicyInput,
    state: State<'_, DbHandle>,
) -> Result<InsurancePolicyView, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    let policy = ops::create_policy(&conn, &input)?;
    ops::policy_view(&conn, policy)
}

#[tauri::command]
pub async fn update_insurance_policy(
    id: i64,
    input: InsurancePolicyInput,
    state: State<'_, DbHandle>,
) -> Result<InsurancePolicyView, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    let policy = ops::update_policy(&conn, id, &input)?;
    ops::policy_view(&conn, policy)
}

#[tauri::command]
pub async fn delete_insurance_policy(
    id: i64,
    confirmed: bool,
    state: State<'_, DbHandle>,
) -> Result<DeleteResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::delete_policy(&conn, id, confirmed)
}

#[tauri::command]
pub async fn assign_firearm_coverage(
    firearm_id: i64,
    input: AssignCoverageInput,
    state: State<'_, DbHandle>,
) -> Result<Firearm, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::assign_firearm_coverage(
        &conn,
        firearm_id,
        input.policy_id,
        input.scheduled_coverage_amount,
    )
}

#[tauri::command]
pub async fn get_value_summary(state: State<'_, DbHandle>) -> Result<ValueSummary, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::get_value_summary(&conn)
}
