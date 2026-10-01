//! specs/006-accessory-links data-model.md "Entity: Accessory": the accessory
//! record, its input and its validation.

use std::collections::HashMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::commands::CommandError;
use crate::models::firearm::{DispositionType, FirearmStatus};
use crate::models::record::RecordRef;
use crate::models::rules::{
    DispositionFields, check_amounts, check_coverage_pair, check_dates_and_disposition,
};
use crate::services::entry_text::{EntryField, check_optional_entry_text};

/// A stored accessory: every column of `accessories`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessory {
    pub id: i64,
    /// FR-019: the record identifier. Set once at creation and never sent
    /// over IPC; only the spreadsheet carries it.
    #[serde(skip)]
    pub uid: String,
    pub accessory_kind_id: i64,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub caliber: Option<String>,
    pub cartridge: Option<String>,
    pub notes: Option<String>,
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
    /// research.md §8: the direct host. Not a column: `from_row` leaves it
    /// `None` and the loaders fill it from `mounts`.
    pub mounted_on: Option<RecordRef>,
    pub created_at: String,
    pub updated_at: String,
}

impl Accessory {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            uid: row.get("uid")?,
            accessory_kind_id: row.get("accessory_kind_id")?,
            make: row.get("make")?,
            model: row.get("model")?,
            serial_number: row.get("serial_number")?,
            caliber: row.get("caliber")?,
            cartridge: row.get("cartridge")?,
            notes: row.get("notes")?,
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
            mounted_on: None,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// All `Accessory` fields except `id`, `uid`, `thumbnailPhotoId` and the
/// timestamps, per contracts/tauri-commands.md's `AccessoryInput`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessoryInput {
    pub accessory_kind_id: i64,
    #[serde(default)]
    pub make: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub caliber: Option<String>,
    #[serde(default)]
    pub cartridge: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub status: FirearmStatus,
    #[serde(default)]
    pub estimated_value: Option<i64>,
    #[serde(default)]
    pub acquisition_source: Option<String>,
    #[serde(default)]
    pub acquisition_date: Option<String>,
    #[serde(default)]
    pub acquisition_price: Option<i64>,
    #[serde(default)]
    pub disposition_type: Option<DispositionType>,
    #[serde(default)]
    pub disposition_recipient: Option<String>,
    #[serde(default)]
    pub disposition_date: Option<String>,
    #[serde(default)]
    pub disposition_price: Option<i64>,
    #[serde(default)]
    pub insurance_policy_id: Option<i64>,
    #[serde(default)]
    pub scheduled_coverage_amount: Option<i64>,
    /// FR-010; checked and saved by the command, as `FirearmInput`'s.
    #[serde(default)]
    pub mounted_on: Option<RecordRef>,
}

/// The record as an input that would save it unchanged — the starting point
/// for commands that change only part of an accessory (dispose, reverse a
/// disposition, assign coverage).
impl From<&Accessory> for AccessoryInput {
    fn from(accessory: &Accessory) -> Self {
        Self {
            accessory_kind_id: accessory.accessory_kind_id,
            make: accessory.make.clone(),
            model: accessory.model.clone(),
            serial_number: accessory.serial_number.clone(),
            caliber: accessory.caliber.clone(),
            cartridge: accessory.cartridge.clone(),
            notes: accessory.notes.clone(),
            status: accessory.status,
            estimated_value: accessory.estimated_value,
            acquisition_source: accessory.acquisition_source.clone(),
            acquisition_date: accessory.acquisition_date.clone(),
            acquisition_price: accessory.acquisition_price,
            disposition_type: accessory.disposition_type,
            disposition_recipient: accessory.disposition_recipient.clone(),
            disposition_date: accessory.disposition_date.clone(),
            disposition_price: accessory.disposition_price,
            insurance_policy_id: accessory.insurance_policy_id,
            scheduled_coverage_amount: accessory.scheduled_coverage_amount,
            mounted_on: accessory.mounted_on,
        }
    }
}

impl AccessoryInput {
    /// The input as it is stored: every text field is trimmed and a blank
    /// one becomes `None`, as for `FirearmInput` (FR-001, FR-003).
    pub fn normalized(&self) -> Self {
        let trimmed = |value: &Option<String>| {
            value.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
        };
        Self {
            make: trimmed(&self.make),
            model: trimmed(&self.model),
            serial_number: trimmed(&self.serial_number),
            caliber: trimmed(&self.caliber),
            cartridge: trimmed(&self.cartridge),
            notes: trimmed(&self.notes),
            acquisition_source: trimmed(&self.acquisition_source),
            acquisition_date: trimmed(&self.acquisition_date),
            disposition_recipient: trimmed(&self.disposition_recipient),
            disposition_date: trimmed(&self.disposition_date),
            ..self.clone()
        }
    }
}

/// data-model.md "Validation rules", shared by the commands and import
/// (FR-024). `stored` is the record an update replaces: 004's entry rules
/// then apply only to a make, model, cartridge or caliber whose trimmed
/// value differs from the stored one, so an existing value over the cap
/// stays valid until that field is edited. Create and import pass `None`.
/// That the kind exists is the command's check (it needs the database).
pub fn validate_accessory_input(
    input: &AccessoryInput,
    stored: Option<&Accessory>,
) -> Result<(), CommandError> {
    let mut errors: HashMap<String, String> = HashMap::new();

    for (field, value, stored_value) in [
        (EntryField::Make, &input.make, stored.map(|a| &a.make)),
        (EntryField::Model, &input.model, stored.map(|a| &a.model)),
        (EntryField::Cartridge, &input.cartridge, stored.map(|a| &a.cartridge)),
        (EntryField::Caliber, &input.caliber, stored.map(|a| &a.caliber)),
    ] {
        let value = value.as_deref().unwrap_or("").trim();
        if stored_value.is_some_and(|stored| stored.as_deref().unwrap_or("").trim() == value) {
            continue;
        }
        if let Err(message) = check_optional_entry_text(field, value) {
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

    // Judged against the user's local date, not UTC. The price stays
    // required for a disposed accessory until User Story 3 (research.md §9).
    let today = chrono::Local::now().date_naive();
    check_dates_and_disposition(
        &DispositionFields {
            noun: "accessory",
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

    check_coverage_pair(input.insurance_policy_id, input.scheduled_coverage_amount, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CommandError::validation("The accessory record has validation errors.", errors))
    }
}
