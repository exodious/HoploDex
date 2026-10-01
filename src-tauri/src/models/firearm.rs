use std::collections::HashMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::commands::CommandError;
use crate::models::record::RecordRef;
use crate::models::rules::{
    DispositionFields, check_amounts, check_coverage_pair, check_dates_and_disposition,
    checked_date, is_blank,
};
use crate::services::entry_text::{EntryField, check_entry_text};

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
    /// specs/006-accessory-links FR-019: the record identifier. Set once at
    /// creation and never sent over IPC; only the spreadsheet carries it.
    #[serde(skip)]
    pub uid: String,
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
    /// specs/005-regulated-item-types FR-007: a `registration_classes` id;
    /// `None` = no classification.
    pub registration_class_id: Option<i64>,
    /// FR-009: e.g. "Form 4". Only with a classification.
    pub registration_form: Option<String>,
    /// FR-009, FR-010: `YYYY-MM-DD`. Only with a classification.
    pub registration_approved: Option<String>,
    /// FR-009: e.g. "Smith Family Trust". Only with a classification.
    pub registered_to: Option<String>,
    /// specs/006-accessory-links research.md §8: the direct host. Not a
    /// column: `from_row` leaves it `None` and the loaders fill it from
    /// `mounts`.
    pub mounted_on: Option<RecordRef>,
    pub created_at: String,
    pub updated_at: String,
}

impl Firearm {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            uid: row.get("uid")?,
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
            registration_class_id: row.get("registration_class_id")?,
            registration_form: row.get("registration_form")?,
            registration_approved: row.get("registration_approved")?,
            registered_to: row.get("registered_to")?,
            mounted_on: None,
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
    /// specs/005-regulated-item-types FR-007: a `registration_classes` id;
    /// `None` = no classification.
    #[serde(default)]
    pub registration_class_id: Option<i64>,
    /// FR-009: e.g. "Form 4". Only with a classification.
    #[serde(default)]
    pub registration_form: Option<String>,
    /// FR-009, FR-010: `YYYY-MM-DD`. Only with a classification.
    #[serde(default)]
    pub registration_approved: Option<String>,
    /// FR-009: e.g. "Smith Family Trust". Only with a classification.
    #[serde(default)]
    pub registered_to: Option<String>,
    /// specs/006-accessory-links research.md §8: the direct host (FR-010);
    /// checked and saved by the command, in the save's savepoint. The form
    /// always sends it.
    #[serde(default)]
    pub mounted_on: Option<RecordRef>,
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
            registration_class_id: firearm.registration_class_id,
            registration_form: firearm.registration_form.clone(),
            registration_approved: firearm.registration_approved.clone(),
            registered_to: firearm.registered_to.clone(),
            mounted_on: firearm.mounted_on,
        }
    }
}

impl FirearmInput {
    /// The input as it is stored: a blank nickname, serial number or finish
    /// becomes `None` and any other is trimmed (FR-031, FR-032, FR-039: blank
    /// is not a value, and comparison ignores surrounding whitespace). Make,
    /// model and caliber are trimmed and a blank cartridge becomes `None`
    /// (specs/004-cartridges-action-types FR-015, research.md §9). The two
    /// registration text details are treated as the cartridge is
    /// (specs/005-regulated-item-types research.md §6).
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
            registration_form: trimmed(&self.registration_form),
            registration_approved: trimmed(&self.registration_approved),
            registered_to: trimmed(&self.registered_to),
            ..self.clone()
        }
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
/// can produce per-row human-readable errors too. `stored` is the record an
/// update replaces: specs/004-cartridges-action-types FR-015's entry rules
/// then apply only to a make, model, cartridge or caliber whose trimmed value
/// differs from the stored one, so an existing value over the cap stays
/// valid until that field is edited (spec Assumptions). Create and import
/// pass `None` and check all four.
pub fn validate_firearm_input(
    input: &FirearmInput,
    stored: Option<&Firearm>,
) -> Result<(), CommandError> {
    let mut errors: HashMap<String, String> = HashMap::new();

    for field in EntryField::ALL {
        let (value, stored_value) = match field {
            EntryField::Make => (input.make.as_str(), stored.map(|f| f.make.as_str())),
            EntryField::Model => (input.model.as_str(), stored.map(|f| f.model.as_str())),
            EntryField::Cartridge => (
                input.cartridge.as_deref().unwrap_or(""),
                stored.map(|f| f.cartridge.as_deref().unwrap_or("")),
            ),
            EntryField::Caliber => (input.caliber.as_str(), stored.map(|f| f.caliber.as_str())),
            EntryField::RegistrationForm => (
                input.registration_form.as_deref().unwrap_or(""),
                stored.map(|f| f.registration_form.as_deref().unwrap_or("")),
            ),
            EntryField::RegisteredTo => (
                input.registered_to.as_deref().unwrap_or(""),
                stored.map(|f| f.registered_to.as_deref().unwrap_or("")),
            ),
        };
        if stored_value.is_some_and(|stored| stored.trim() == value.trim()) {
            continue;
        }
        if let Err(message) = check_entry_text(field, value) {
            errors.insert(field.ipc_name().into(), message);
        }
    }

    check_amounts(
        input.estimated_value,
        input.acquisition_price,
        input.disposition_price,
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
    check_dates_and_disposition(
        &DispositionFields {
            noun: "firearm",
            status: input.status,
            acquisition_date: &input.acquisition_date,
            disposition_type: input.disposition_type,
            disposition_recipient: &input.disposition_recipient,
            disposition_date: &input.disposition_date,
            disposition_price: input.disposition_price,
            price_required: true,
        },
        today,
        &mut errors,
    );

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

    // specs/005-regulated-item-types FR-009, FR-010: registration details
    // need a classification, and the approved date is not in the future.
    // Only the classification's existence is checked by the command.
    checked_date(
        "registrationApproved",
        "Approved date",
        &input.registration_approved,
        today,
        &mut errors,
    );
    if input.registration_class_id.is_none() {
        for (field, blank) in [
            ("registrationForm", is_blank(&input.registration_form)),
            ("registrationApproved", is_blank(&input.registration_approved)),
            ("registeredTo", is_blank(&input.registered_to)),
        ] {
            if !blank {
                errors
                    .insert(field.into(), "Choose what the firearm is registered as first.".into());
            }
        }
    }

    check_coverage_pair(input.insurance_policy_id, input.scheduled_coverage_amount, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CommandError::validation("The firearm record has validation errors.", errors))
    }
}
