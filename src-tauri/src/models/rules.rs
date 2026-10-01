//! The amount, date, disposition and insurance-pair rules a firearm and an
//! accessory share (specs/006-accessory-links tasks.md T034, data-model.md
//! "Validation rules"), so the two records can never judge them differently.
//! Each check records its message under the IPC field name in `errors`; the
//! callers turn a non-empty map into the `VALIDATION_ERROR`.

use std::collections::HashMap;

use chrono::NaiveDate;

use crate::models::firearm::{DispositionType, FirearmStatus};

pub(crate) type FieldErrors = HashMap<String, String>;

pub(crate) fn is_blank(value: &Option<String>) -> bool {
    value.as_deref().map(str::trim).unwrap_or("").is_empty()
}

/// Parses an optional `YYYY-MM-DD` date that must not be later than
/// `today` (FR-003/FR-004: today is allowed). A blank date is fine — both
/// dates are optional — and yields `None`; a bad one records an error under
/// `field` and also yields `None`.
pub(crate) fn checked_date(
    field: &str,
    label: &str,
    value: &Option<String>,
    today: NaiveDate,
    errors: &mut FieldErrors,
) -> Option<NaiveDate> {
    if is_blank(value) {
        return None;
    }
    match NaiveDate::parse_from_str(value.as_deref().unwrap_or_default().trim(), "%Y-%m-%d") {
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
pub(crate) fn checked_amount(
    field: &str,
    label: &str,
    value: Option<i64>,
    errors: &mut FieldErrors,
) {
    if value.is_some_and(|dollars| dollars < 0) {
        errors.insert(field.into(), format!("{label} can't be negative."));
    }
}

/// The four amounts every record has.
pub(crate) fn check_amounts(
    estimated_value: Option<i64>,
    acquisition_price: Option<i64>,
    disposition_price: Option<i64>,
    scheduled_coverage_amount: Option<i64>,
    errors: &mut FieldErrors,
) {
    checked_amount("estimatedValue", "Estimated value", estimated_value, errors);
    checked_amount("acquisitionPrice", "Acquisition price", acquisition_price, errors);
    checked_amount("dispositionPrice", "Disposition price", disposition_price, errors);
    checked_amount(
        "scheduledCoverageAmount",
        "Scheduled coverage amount",
        scheduled_coverage_amount,
        errors,
    );
}

/// What the date and disposition rules read from a record.
pub(crate) struct DispositionFields<'a> {
    /// "firearm" or "accessory", for the message that names the record.
    pub noun: &'a str,
    pub status: FirearmStatus,
    pub acquisition_date: &'a Option<String>,
    pub disposition_type: Option<DispositionType>,
    pub disposition_recipient: &'a Option<String>,
    pub disposition_date: &'a Option<String>,
    pub disposition_price: Option<i64>,
    /// Whether a disposed record must have a price.
    pub price_required: bool,
}

/// FR-003/FR-004: both dates are judged against the user's local date, and
/// the disposition date is not before the acquisition date. A disposed
/// record needs its type, recipient and date (and its price when
/// `price_required`), and an active one none of them.
pub(crate) fn check_dates_and_disposition(
    fields: &DispositionFields<'_>,
    today: NaiveDate,
    errors: &mut FieldErrors,
) {
    let acquired =
        checked_date("acquisitionDate", "Acquisition date", fields.acquisition_date, today, errors);
    let disposed_on =
        checked_date("dispositionDate", "Disposition date", fields.disposition_date, today, errors);
    if let (Some(acquired), Some(disposed_on)) = (acquired, disposed_on)
        && disposed_on < acquired
    {
        errors.insert(
            "dispositionDate".into(),
            "Disposition date can't be earlier than the acquisition date.".into(),
        );
    }

    match fields.status {
        FirearmStatus::Disposed => {
            let required = "Required when marking as disposed.";
            if fields.disposition_type.is_none() {
                errors.insert("dispositionType".into(), required.into());
            }
            if is_blank(fields.disposition_recipient) {
                errors.insert("dispositionRecipient".into(), required.into());
            }
            if is_blank(fields.disposition_date) {
                errors.insert("dispositionDate".into(), required.into());
            }
            if fields.price_required && fields.disposition_price.is_none() {
                errors.insert("dispositionPrice".into(), required.into());
            }
        }
        FirearmStatus::Active => {
            if fields.disposition_type.is_some()
                || !is_blank(fields.disposition_recipient)
                || !is_blank(fields.disposition_date)
                || fields.disposition_price.is_some()
            {
                errors.insert(
                    "status".into(),
                    format!(
                        "Disposition details can only be set once a {} is marked disposed.",
                        fields.noun
                    ),
                );
            }
        }
    }
}

/// FR-014/FR-036: scheduled under a policy with its own amount, or not at
/// all. There is no per-record blanket assignment.
pub(crate) fn check_coverage_pair(
    insurance_policy_id: Option<i64>,
    scheduled_coverage_amount: Option<i64>,
    errors: &mut FieldErrors,
) {
    match (insurance_policy_id, scheduled_coverage_amount) {
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
}
