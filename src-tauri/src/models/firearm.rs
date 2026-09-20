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
    pub nickname: Option<String>,
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
            nickname: row.get("nickname")?,
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
    pub nickname: Option<String>,
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
    pub scheduled_coverage_amount: Option<i64>,
}

/// The record as an input that would save it unchanged — the starting point
/// for commands that change only part of a firearm (dispose, reverse a
/// disposition, assign coverage).
impl From<&Firearm> for FirearmInput {
    fn from(firearm: &Firearm) -> Self {
        Self {
            make: firearm.make.clone(),
            model: firearm.model.clone(),
            serial_number: firearm.serial_number.clone(),
            no_serial_attested: firearm.no_serial_attested,
            caliber: firearm.caliber.clone(),
            firearm_type_id: firearm.firearm_type_id,
            nickname: firearm.nickname.clone(),
            notes: firearm.notes.clone(),
            accessories: firearm.accessories.clone(),
            status: firearm.status,
            estimated_value: firearm.estimated_value,
            acquisition_source: firearm.acquisition_source.clone(),
            acquisition_date: firearm.acquisition_date.clone(),
            acquisition_price: firearm.acquisition_price,
            disposition_type: firearm.disposition_type,
            disposition_recipient: firearm.disposition_recipient.clone(),
            disposition_date: firearm.disposition_date.clone(),
            disposition_price: firearm.disposition_price,
            insurance_policy_id: firearm.insurance_policy_id,
            scheduled_coverage_amount: firearm.scheduled_coverage_amount,
        }
    }
}

impl FirearmInput {
    /// The input as it is stored: a blank nickname becomes `None` and any
    /// other is trimmed (FR-031: blank is not a value, and comparison
    /// ignores surrounding whitespace).
    pub fn normalized(&self) -> Self {
        let nickname = self.nickname.as_deref().map(str::trim).filter(|n| !n.is_empty());
        Self { nickname: nickname.map(str::to_owned), ..self.clone() }
    }
}

fn is_blank(value: &Option<String>) -> bool {
    value.as_deref().map(str::trim).unwrap_or("").is_empty()
}

/// Parses an optional `YYYY-MM-DD` date that must not be later than
/// `today` (FR-003/FR-004: today is allowed). A blank date is fine — both
/// dates are optional — and yields `None`; a bad one records an error under
/// `field` and also yields `None`.
fn checked_date(
    field: &str,
    label: &str,
    value: &Option<String>,
    today: chrono::NaiveDate,
    errors: &mut HashMap<String, String>,
) -> Option<chrono::NaiveDate> {
    if is_blank(value) {
        return None;
    }
    match chrono::NaiveDate::parse_from_str(value.as_deref().unwrap_or_default().trim(), "%Y-%m-%d")
    {
        Ok(date) if date > today => {
            errors.insert(field.into(), format!("{label} can't be in the future."));
            None
        }
        Ok(date) => Some(date),
        Err(_) => {
            errors.insert(field.into(), format!("{label} must be a date in YYYY-MM-DD format."));
            None
        }
    }
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

    // FR-003/FR-004: judged against the user's local date, not UTC.
    let today = chrono::Local::now().date_naive();
    let acquired = checked_date(
        "acquisitionDate",
        "Acquisition date",
        &input.acquisition_date,
        today,
        &mut errors,
    );
    let disposed_on = checked_date(
        "dispositionDate",
        "Disposition date",
        &input.disposition_date,
        today,
        &mut errors,
    );
    if let (Some(acquired), Some(disposed_on)) = (acquired, disposed_on) {
        if disposed_on < acquired {
            errors.insert(
                "dispositionDate".into(),
                "Disposition date can't be earlier than the acquisition date.".into(),
            );
        }
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

    // FR-014/FR-036: scheduled under a policy with its own amount, or not at
    // all. There is no per-firearm blanket assignment.
    match (input.insurance_policy_id, input.scheduled_coverage_amount) {
        (Some(_), None) => {
            errors.insert(
                "scheduledCoverageAmount".into(),
                "Enter the amount scheduled on the policy.".into(),
            );
        }
        (None, Some(_)) => {
            errors.insert(
                "insurancePolicyId".into(),
                "Choose the policy this amount is scheduled on.".into(),
            );
        }
        _ => {}
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CommandError::validation("The firearm record has validation errors.", errors))
    }
}
