use std::collections::HashMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::commands::CommandError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsurancePolicy {
    pub id: i64,
    pub name: String,
    pub policy_number: String,
    pub insurance_company: String,
    pub company_contact: Option<String>,
    pub agent_name: Option<String>,
    pub agent_contact: Option<String>,
    pub notes: Option<String>,
    /// Set for a blanket policy (FR-036), absent for a schedule-only one.
    pub blanket_coverage_limit: Option<i64>,
    pub effective_start_date: String,
    pub effective_end_date: String,
    pub created_at: String,
    pub updated_at: String,
}

impl InsurancePolicy {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            policy_number: row.get("policy_number")?,
            insurance_company: row.get("insurance_company")?,
            company_contact: row.get("company_contact")?,
            agent_name: row.get("agent_name")?,
            agent_contact: row.get("agent_contact")?,
            notes: row.get("notes")?,
            blanket_coverage_limit: row.get("blanket_coverage_limit")?,
            effective_start_date: row.get("effective_start_date")?,
            effective_end_date: row.get("effective_end_date")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// All `InsurancePolicy` fields except `id`, `createdAt`, `updatedAt`, per
/// contracts/tauri-commands.md's `InsurancePolicyInput`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsurancePolicyInput {
    pub name: String,
    pub policy_number: String,
    pub insurance_company: String,
    pub company_contact: Option<String>,
    pub agent_name: Option<String>,
    pub agent_contact: Option<String>,
    pub notes: Option<String>,
    /// Set for a blanket policy (FR-036), absent for a schedule-only one.
    pub blanket_coverage_limit: Option<i64>,
    pub effective_start_date: String,
    pub effective_end_date: String,
}

impl InsurancePolicyInput {
    /// Notes as stored: trimmed, and a blank or whitespace-only entry is no
    /// notes at all (FR-027).
    pub fn normalized_notes(&self) -> Option<&str> {
        self.notes.as_deref().map(str::trim).filter(|notes| !notes.is_empty())
    }
}

/// Validation rules from data-model.md's InsurancePolicy section.
pub fn validate_insurance_policy_input(input: &InsurancePolicyInput) -> Result<(), CommandError> {
    let mut errors: HashMap<String, String> = HashMap::new();

    if input.name.trim().is_empty() {
        errors.insert("name".into(), "Name is required.".into());
    }
    if input.policy_number.trim().is_empty() {
        errors.insert("policyNumber".into(), "Policy number is required.".into());
    }
    if input.insurance_company.trim().is_empty() {
        errors.insert("insuranceCompany".into(), "Insurance company is required.".into());
    }
    if input.blanket_coverage_limit.is_some_and(|limit| limit < 0) {
        errors.insert(
            "blanketCoverageLimit".into(),
            "Blanket coverage limit cannot be negative.".into(),
        );
    }

    match (
        chrono::NaiveDate::parse_from_str(&input.effective_start_date, "%Y-%m-%d"),
        chrono::NaiveDate::parse_from_str(&input.effective_end_date, "%Y-%m-%d"),
    ) {
        (Ok(start), Ok(end)) if end <= start => {
            errors
                .insert("effectiveEndDate".into(), "End date must be after the start date.".into());
        }
        (Ok(_), Ok(_)) => {}
        _ => {
            errors
                .insert("effectiveStartDate".into(), "Dates must be in YYYY-MM-DD format.".into());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CommandError::validation("The insurance policy has validation errors.", errors))
    }
}
