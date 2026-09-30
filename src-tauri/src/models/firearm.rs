use std::collections::HashMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::commands::CommandError;

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

text_enum!(Condition {
    NewInBox => "new_in_box",
    LikeNew => "like_new",
    Excellent => "excellent",
    Good => "good",
    Fair => "fair",
    Poor => "poor",
});

text_enum!(Origin {
    Domestic => "domestic",
    Imported => "imported",
    Reimported => "reimported",
});

impl Origin {
    /// specs/002-firearm-identification FR-001/FR-012: how the origin is
    /// shown to the user, exported, and indexed for search. Kept in step
    /// with the `CASE` in `0002_fts5.sql` and `ORIGIN_OPTIONS` in
    /// `src/features/firearms/types.ts` (research.md §6).
    pub fn label(&self) -> &'static str {
        match self {
            Self::Domestic => "Domestic",
            Self::Imported => "Imported",
            Self::Reimported => "Re-imported",
        }
    }
}

impl Condition {
    /// Every grade, best first.
    pub const ALL: [Condition; 6] =
        [Self::NewInBox, Self::LikeNew, Self::Excellent, Self::Good, Self::Fair, Self::Poor];

    /// How the grade is shown to the user, and exported (FR-039).
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewInBox => "New in box",
            Self::LikeNew => "Like new",
            Self::Excellent => "Excellent",
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::Poor => "Poor",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Firearm {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub serial_number: Option<String>,
    pub no_serial_attested: bool,
    pub caliber: String,
    /// specs/004-cartridges-action-types FR-001: the exact round; `None` =
    /// none recorded.
    pub cartridge: Option<String>,
    pub firearm_type_id: i64,
    /// specs/004-cartridges-action-types FR-017: an `action_types` id; `None`
    /// = not specified.
    pub action_type_id: Option<i64>,
    pub nickname: Option<String>,
    pub notes: Option<String>,
    pub accessories: Option<String>,
    /// FR-039: hundredths of an inch.
    pub barrel_length_hundredths: Option<i64>,
    /// FR-039: hundredths of an inch.
    pub overall_length_hundredths: Option<i64>,
    /// FR-039: tenths of an ounce.
    pub weight_tenths_oz: Option<i64>,
    /// FR-039: rounds the magazine, cylinder or tube holds.
    pub capacity: Option<i64>,
    pub finish: Option<String>,
    pub condition: Option<Condition>,
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
    /// specs/002-firearm-identification FR-001: `None` means not specified.
    pub origin: Option<Origin>,
    /// specs/002-firearm-identification FR-003.
    pub year_of_manufacture: Option<i64>,
    /// specs/002-firearm-identification FR-002: only for `Origin::Imported`.
    pub country_of_manufacture: Option<String>,
    /// specs/002-firearm-identification FR-002: import-marked origins only.
    pub importer_name: Option<String>,
    /// specs/002-firearm-identification FR-004: import-marked origins only.
    pub original_make: Option<String>,
    pub original_model: Option<String>,
    pub original_serial_number: Option<String>,
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
            cartridge: row.get("cartridge")?,
            firearm_type_id: row.get("firearm_type_id")?,
            action_type_id: row.get("action_type_id")?,
            nickname: row.get("nickname")?,
            notes: row.get("notes")?,
            accessories: row.get("accessories")?,
            barrel_length_hundredths: row.get("barrel_length_hundredths")?,
            overall_length_hundredths: row.get("overall_length_hundredths")?,
            weight_tenths_oz: row.get("weight_tenths_oz")?,
            capacity: row.get("capacity")?,
            finish: row.get("finish")?,
            condition: row.get("condition")?,
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
            origin: row.get("origin")?,
            year_of_manufacture: row.get("year_of_manufacture")?,
            country_of_manufacture: row.get("country_of_manufacture")?,
            importer_name: row.get("importer_name")?,
            original_make: row.get("original_make")?,
            original_model: row.get("original_model")?,
            original_serial_number: row.get("original_serial_number")?,
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
    /// specs/004-cartridges-action-types FR-001: the exact round; `None` =
    /// none recorded.
    #[serde(default)]
    pub cartridge: Option<String>,
    pub firearm_type_id: i64,
    /// specs/004-cartridges-action-types FR-017: an `action_types` id; `None`
    /// = not specified.
    #[serde(default)]
    pub action_type_id: Option<i64>,
    pub nickname: Option<String>,
    pub notes: Option<String>,
    pub accessories: Option<String>,
    /// FR-039: hundredths of an inch.
    pub barrel_length_hundredths: Option<i64>,
    /// FR-039: hundredths of an inch.
    pub overall_length_hundredths: Option<i64>,
    /// FR-039: tenths of an ounce.
    pub weight_tenths_oz: Option<i64>,
    /// FR-039: rounds the magazine, cylinder or tube holds.
    pub capacity: Option<i64>,
    pub finish: Option<String>,
    pub condition: Option<Condition>,
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
    /// specs/002-firearm-identification FR-001: `None` means not specified.
    pub origin: Option<Origin>,
    /// specs/002-firearm-identification FR-003.
    pub year_of_manufacture: Option<i64>,
    /// specs/002-firearm-identification FR-002: only for `Origin::Imported`.
    pub country_of_manufacture: Option<String>,
    /// specs/002-firearm-identification FR-002: import-marked origins only.
    pub importer_name: Option<String>,
    /// specs/002-firearm-identification FR-004: import-marked origins only.
    pub original_make: Option<String>,
    pub original_model: Option<String>,
    pub original_serial_number: Option<String>,
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
            cartridge: firearm.cartridge.clone(),
            firearm_type_id: firearm.firearm_type_id,
            action_type_id: firearm.action_type_id,
            nickname: firearm.nickname.clone(),
            notes: firearm.notes.clone(),
            accessories: firearm.accessories.clone(),
            barrel_length_hundredths: firearm.barrel_length_hundredths,
            overall_length_hundredths: firearm.overall_length_hundredths,
            weight_tenths_oz: firearm.weight_tenths_oz,
            capacity: firearm.capacity,
            finish: firearm.finish.clone(),
            condition: firearm.condition,
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
            origin: firearm.origin,
            year_of_manufacture: firearm.year_of_manufacture,
            country_of_manufacture: firearm.country_of_manufacture.clone(),
            importer_name: firearm.importer_name.clone(),
            original_make: firearm.original_make.clone(),
            original_model: firearm.original_model.clone(),
            original_serial_number: firearm.original_serial_number.clone(),
        }
    }
}

impl FirearmInput {
    /// The input as it is stored: a blank nickname, serial number or finish
    /// becomes `None` and any other is trimmed (FR-031, FR-032, FR-039: blank
    /// is not a value, and comparison ignores surrounding whitespace). Make,
    /// model and caliber are trimmed and a blank cartridge becomes `None`
    /// (specs/004-cartridges-action-types FR-015, research.md §9).
    pub fn normalized(&self) -> Self {
        let trimmed = |value: &Option<String>| {
            value.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
        };
        Self {
            make: self.make.trim().to_owned(),
            model: self.model.trim().to_owned(),
            caliber: self.caliber.trim().to_owned(),
            cartridge: trimmed(&self.cartridge),
            nickname: trimmed(&self.nickname),
            serial_number: trimmed(&self.serial_number),
            finish: trimmed(&self.finish),
            country_of_manufacture: trimmed(&self.country_of_manufacture),
            importer_name: trimmed(&self.importer_name),
            original_make: trimmed(&self.original_make),
            original_model: trimmed(&self.original_model),
            original_serial_number: trimmed(&self.original_serial_number),
            ..self.clone()
        }
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

/// FR-037: an amount is a whole number of dollars, so once it has decoded
/// (a fractional number never does) the only thing left to refuse is a
/// negative one.
fn checked_amount(
    field: &str,
    label: &str,
    value: Option<i64>,
    errors: &mut HashMap<String, String>,
) {
    if value.is_some_and(|dollars| dollars < 0) {
        errors.insert(field.into(), format!("{label} can't be negative."));
    }
}

/// FR-039: a length, weight or capacity, once it has decoded as a whole
/// number (a fractional one never does), must be at least `min`.
fn checked_measure(
    field: &str,
    message: &str,
    value: Option<i64>,
    min: i64,
    errors: &mut HashMap<String, String>,
) {
    if value.is_some_and(|measure| measure < min) {
        errors.insert(field.into(), message.into());
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

    checked_amount("estimatedValue", "Estimated value", input.estimated_value, &mut errors);
    checked_amount("acquisitionPrice", "Acquisition price", input.acquisition_price, &mut errors);
    checked_amount("dispositionPrice", "Disposition price", input.disposition_price, &mut errors);
    checked_amount(
        "scheduledCoverageAmount",
        "Scheduled coverage amount",
        input.scheduled_coverage_amount,
        &mut errors,
    );

    let positive = |what: &str| format!("{what} must be greater than 0.");
    checked_measure(
        "barrelLengthHundredths",
        &positive("Barrel length"),
        input.barrel_length_hundredths,
        1,
        &mut errors,
    );
    checked_measure(
        "overallLengthHundredths",
        &positive("Overall length"),
        input.overall_length_hundredths,
        1,
        &mut errors,
    );
    checked_measure("weightTenthsOz", &positive("Weight"), input.weight_tenths_oz, 1, &mut errors);
    checked_measure("capacity", "Capacity must be at least 1.", input.capacity, 1, &mut errors);

    // Acceptance Scenarios 6-7 and FR-029: a serial number or the attestation
    // that there is none, never both and never neither.
    if is_blank(&input.serial_number) && !input.no_serial_attested {
        errors.insert(
            "serialNumber".into(),
            "Enter a serial number, or confirm this firearm has none.".into(),
        );
    } else if !is_blank(&input.serial_number) && input.no_serial_attested {
        errors.insert(
            "serialNumber".into(),
            "A firearm with no serial number can't also have one. Clear the serial number, \
             or uncheck the box."
                .into(),
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
    if let (Some(acquired), Some(disposed_on)) = (acquired, disposed_on)
        && disposed_on < acquired
    {
        errors.insert(
            "dispositionDate".into(),
            "Disposition date can't be earlier than the acquisition date.".into(),
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

    // specs/002-firearm-identification FR-003: a whole four-digit year, no
    // later than the user's local current year. `checked_measure`'s `>= min`
    // shape doesn't fit an upper bound too, so this is spelled out.
    if let Some(year) = input.year_of_manufacture {
        let current_year = chrono::Local::now()
            .date_naive()
            .format("%Y")
            .to_string()
            .parse::<i64>()
            .unwrap_or(9999);
        if year < 1400 || year > current_year {
            errors.insert(
                "yearOfManufacture".into(),
                format!(
                    "Year of manufacture must be a four-digit year from 1400 to {current_year}."
                ),
            );
        }
    }

    // specs/002-firearm-identification FR-002: the importer's name and
    // country of manufacture only apply to import-marked origins.
    let import_marked = matches!(input.origin, Some(Origin::Imported) | Some(Origin::Reimported));
    if !is_blank(&input.importer_name) && !import_marked {
        errors.insert(
            "importerName".into(),
            "Importer applies only to an imported or re-imported firearm.".into(),
        );
    }
    if !is_blank(&input.country_of_manufacture) && !matches!(input.origin, Some(Origin::Imported)) {
        let message = if matches!(input.origin, Some(Origin::Reimported)) {
            "Country of manufacture applies only to imported firearms; a re-imported firearm is \
             made in the United States."
        } else {
            "Country of manufacture applies only to imported firearms."
        };
        errors.insert("countryOfManufacture".into(), message.into());
    }

    // specs/002-firearm-identification FR-004: the original manufacturer's
    // marks only apply to import-marked origins, same gating as importer_name.
    for (field, label, value) in [
        ("originalMake", "Original maker", &input.original_make),
        ("originalModel", "Original model", &input.original_model),
        ("originalSerialNumber", "Original serial number", &input.original_serial_number),
    ] {
        if !is_blank(value) && !import_marked {
            errors.insert(
                field.into(),
                format!("{label} applies only to an imported or re-imported firearm."),
            );
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
