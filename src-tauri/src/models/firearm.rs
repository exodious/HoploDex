use std::collections::HashMap;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::commands::CommandError;

macro_rules! text_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                let text = value.as_str()?;
                match text {
                    $($text => Ok(Self::$variant)),+,
                    other => Err(FromSqlError::Other(
                        format!("unrecognized {} value: {other}", stringify!($name)).into(),
                    )),
                }
            }
        }
    };
}

text_enum!(FirearmStatus {
    Active => "active",
    Disposed => "disposed",
});

text_enum!(DispositionType {
    Sold => "sold",
    Traded => "traded",
    Gifted => "gifted",
    Destroyed => "destroyed",
    LostStolen => "lost_stolen",
});

text_enum!(CoverageKind {
    IndividuallyScheduled => "individually_scheduled",
    Blanket => "blanket",
});

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Firearm {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub serial_number: Option<String>,
    pub no_serial_attested: bool,
    pub caliber: String,
    pub firearm_type_id: i64,
    pub notes: Option<String>,
    pub accessories: Option<String>,
    pub status: FirearmStatus,
    pub estimated_value: Option<i64>,
    pub acquisition_source: Option<String>,
    pub acquisition_date: Option<String>,
    pub acquisition_price: Option<i64>,
    pub disposition_type: Option<DispositionType>,
    pub disposition_recipient: Option<String>,
    pub disposition_date: Option<String>,
    pub disposition_price: Option<i64>,
    pub thumbnail_photo_id: Option<i64>,
    pub insurance_policy_id: Option<i64>,
    pub coverage_kind: Option<CoverageKind>,
    pub scheduled_coverage_amount: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl Firearm {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            make: row.get("make")?,
            model: row.get("model")?,
            serial_number: row.get("serial_number")?,
            no_serial_attested: row.get("no_serial_attested")?,
            caliber: row.get("caliber")?,
            firearm_type_id: row.get("firearm_type_id")?,
            notes: row.get("notes")?,
            accessories: row.get("accessories")?,
            status: row.get("status")?,
            estimated_value: row.get("estimated_value")?,
            acquisition_source: row.get("acquisition_source")?,
            acquisition_date: row.get("acquisition_date")?,
            acquisition_price: row.get("acquisition_price")?,
            disposition_type: row.get("disposition_type")?,
            disposition_recipient: row.get("disposition_recipient")?,
            disposition_date: row.get("disposition_date")?,
            disposition_price: row.get("disposition_price")?,
            thumbnail_photo_id: row.get("thumbnail_photo_id")?,
            insurance_policy_id: row.get("insurance_policy_id")?,
            coverage_kind: row.get("coverage_kind")?,
            scheduled_coverage_amount: row.get("scheduled_coverage_amount")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// All `Firearm` fields except `id`, `createdAt`, `updatedAt`, and
/// `thumbnailPhotoId`, per contracts/tauri-commands.md's `FirearmInput`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmInput {
    pub make: String,
    pub model: String,
    pub serial_number: Option<String>,
    pub no_serial_attested: bool,
    pub caliber: String,
    pub firearm_type_id: i64,
    pub notes: Option<String>,
    pub accessories: Option<String>,
    pub status: FirearmStatus,
    pub estimated_value: Option<i64>,
    pub acquisition_source: Option<String>,
    pub acquisition_date: Option<String>,
    pub acquisition_price: Option<i64>,
    pub disposition_type: Option<DispositionType>,
    pub disposition_recipient: Option<String>,
    pub disposition_date: Option<String>,
    pub disposition_price: Option<i64>,
    pub insurance_policy_id: Option<i64>,
    pub coverage_kind: Option<CoverageKind>,
    pub scheduled_coverage_amount: Option<i64>,
}

fn is_blank(value: &Option<String>) -> bool {
    value.as_deref().map(str::trim).unwrap_or("").is_empty()
}

/// Validation rules from data-model.md's "Validation rules" section,
/// enforced here (not just at the DB level) so import validation (FR-020)
/// can produce per-row human-readable errors too.
pub fn validate_firearm_input(input: &FirearmInput) -> Result<(), CommandError> {
    let mut errors: HashMap<String, String> = HashMap::new();

    if input.make.trim().is_empty() {
        errors.insert("make".into(), "Make is required.".into());
    }
    if input.model.trim().is_empty() {
        errors.insert("model".into(), "Model is required.".into());
    }
    if input.caliber.trim().is_empty() {
        errors.insert("caliber".into(), "Caliber is required.".into());
    }

    // Acceptance Scenarios 6-7: blank serial number is only allowed when
    // explicitly attested; providing a serial number is always fine.
    if is_blank(&input.serial_number) && !input.no_serial_attested {
        errors.insert(
            "serialNumber".into(),
            "Enter a serial number, or confirm this firearm has none.".into(),
        );
    }

    match input.status {
        FirearmStatus::Disposed => {
            if input.disposition_type.is_none() {
                errors
                    .insert("dispositionType".into(), "Required when marking as disposed.".into());
            }
            if is_blank(&input.disposition_recipient) {
                errors.insert(
                    "dispositionRecipient".into(),
                    "Required when marking as disposed.".into(),
                );
            }
            if is_blank(&input.disposition_date) {
                errors
                    .insert("dispositionDate".into(), "Required when marking as disposed.".into());
            }
            if input.disposition_price.is_none() {
                errors
                    .insert("dispositionPrice".into(), "Required when marking as disposed.".into());
            }
        }
        FirearmStatus::Active => {
            if input.disposition_type.is_some()
                || !is_blank(&input.disposition_recipient)
                || !is_blank(&input.disposition_date)
                || input.disposition_price.is_some()
            {
                errors.insert(
                    "status".into(),
                    "Disposition details can only be set once a firearm is marked disposed.".into(),
                );
            }
        }
    }

    match input.coverage_kind {
        Some(CoverageKind::IndividuallyScheduled) => {
            if input.insurance_policy_id.is_none() {
                errors.insert(
                    "insurancePolicyId".into(),
                    "Select a policy for individually-scheduled coverage.".into(),
                );
            }
            if input.scheduled_coverage_amount.is_none() {
                errors.insert(
                    "scheduledCoverageAmount".into(),
                    "Enter a scheduled coverage amount.".into(),
                );
            }
        }
        Some(CoverageKind::Blanket) => {
            if input.insurance_policy_id.is_none() {
                errors.insert(
                    "insurancePolicyId".into(),
                    "Select a policy for blanket coverage.".into(),
                );
            }
            if input.scheduled_coverage_amount.is_some() {
                errors.insert(
                    "scheduledCoverageAmount".into(),
                    "Blanket-covered firearms draw from the policy's shared limit, not an individual amount.".into(),
                );
            }
        }
        None => {
            if input.insurance_policy_id.is_some() {
                errors.insert(
                    "coverageKind".into(),
                    "Select how this firearm is covered by the assigned policy.".into(),
                );
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CommandError::validation("The firearm record has validation errors.", errors))
    }
}
