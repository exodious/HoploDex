use rusqlite::{named_params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::commands::firearms::{self, DeleteResult};
use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::firearm::{CoverageKind, Firearm};
use crate::models::insurance_policy::{
    validate_insurance_policy_input, InsurancePolicy, InsurancePolicyInput,
};
use crate::services::valuation::{self, ValueSummary};

/// Input for `assign_firearm_coverage`, per contracts/tauri-commands.md.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignCoverageInput {
    pub policy_id: Option<i64>,
    pub coverage_kind: Option<CoverageKind>,
    pub scheduled_coverage_amount: Option<i64>,
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

    pub fn create_policy(
        conn: &Connection,
        input: &InsurancePolicyInput,
    ) -> Result<InsurancePolicy, CommandError> {
        validate_insurance_policy_input(input)?;
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

    pub fn assign_firearm_coverage(
        conn: &Connection,
        firearm_id: i64,
        policy_id: Option<i64>,
        coverage_kind: Option<CoverageKind>,
        scheduled_coverage_amount: Option<i64>,
    ) -> Result<Firearm, CommandError> {
        let current = firearms::ops::get_firearm(conn, firearm_id)?;
        let mut input = crate::models::firearm::FirearmInput {
            make: current.make,
            model: current.model,
            serial_number: current.serial_number,
            no_serial_attested: current.no_serial_attested,
            caliber: current.caliber,
            firearm_type_id: current.firearm_type_id,
            notes: current.notes,
            accessories: current.accessories,
            status: current.status,
            estimated_value: current.estimated_value,
            acquisition_source: current.acquisition_source,
            acquisition_date: current.acquisition_date,
            acquisition_price: current.acquisition_price,
            disposition_type: current.disposition_type,
            disposition_recipient: current.disposition_recipient,
            disposition_date: current.disposition_date,
            disposition_price: current.disposition_price,
            insurance_policy_id: policy_id,
            coverage_kind,
            scheduled_coverage_amount,
        };
        if policy_id.is_none() {
            input.coverage_kind = None;
            input.scheduled_coverage_amount = None;
        }
        firearms::ops::update_firearm(conn, firearm_id, &input)
    }

    pub fn get_value_summary(conn: &Connection) -> Result<ValueSummary, CommandError> {
        valuation::get_value_summary(conn)
    }
}

#[tauri::command]
pub async fn list_insurance_policies(
    state: State<'_, DbHandle>,
) -> Result<Vec<InsurancePolicy>, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::list_policies(&conn)
}

#[tauri::command]
pub async fn create_insurance_policy(
    input: InsurancePolicyInput,
    state: State<'_, DbHandle>,
) -> Result<InsurancePolicy, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::create_policy(&conn, &input)
}

#[tauri::command]
pub async fn update_insurance_policy(
    id: i64,
    input: InsurancePolicyInput,
    state: State<'_, DbHandle>,
) -> Result<InsurancePolicy, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::update_policy(&conn, id, &input)
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
        input.coverage_kind,
        input.scheduled_coverage_amount,
    )
}

#[tauri::command]
pub async fn get_value_summary(state: State<'_, DbHandle>) -> Result<ValueSummary, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::get_value_summary(&conn)
}
