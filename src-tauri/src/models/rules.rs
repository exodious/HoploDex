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

/// The most characters a short free-text field may hold: a nickname, a
/// serial number, a finish, a source or recipient, a firearm's origin
/// details, and a policy's names, number and contacts (specs/008-hardening-batch
/// research.md §10). The entry fields keep their own 100
/// (`services::entry_text::MAX_ENTRY_CHARS`). The frontend repeats it as
/// `MAX_SHORT_TEXT_CHARS` in `src/lib/textLimits.ts`; the two must agree.
pub const MAX_SHORT_TEXT_CHARS: usize = 200;

/// The most characters a long free-text field may hold: notes, and a
/// firearm's accessories text. Mirrored as `MAX_LONG_TEXT_CHARS` in
/// `src/lib/textLimits.ts`.
pub const MAX_LONG_TEXT_CHARS: usize = 4_000;

/// `1,234,567`: a number as the app's messages write it.
pub(crate) fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Once trimmed, `value` is at most `max` characters (Unicode scalar values,
/// as `check_entry_text` counts them, so a four-byte character is one).
/// The error is the message to show under the field.
pub(crate) fn check_text_length(label: &str, value: &str, max: usize) -> Result<(), String> {
    if value.trim().chars().count() > max {
        return Err(format!("{label} can be at most {} characters.", thousands(max as u64)));
    }
    Ok(())
}

/// [`check_text_length`] for an optional field, recording a failure under
/// the IPC name `field`.
pub(crate) fn check_optional_text_length(
    field: &str,
    label: &str,
    value: &Option<String>,
    max: usize,
    errors: &mut FieldErrors,
) {
    if let Err(message) = check_text_length(label, value.as_deref().unwrap_or(""), max) {
        errors.insert(field.into(), message);
    }
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

/// The largest amount any single money field may hold, in whole dollars
/// (#67). The frontend repeats it as `MAX_DOLLARS` in `src/lib/money.ts`; the
/// two must agree. It is a rule on what is *entered*: totals never rely on
/// it, because a row written before the cap existed may be larger (see
/// [`saturating_total`]).
pub const MAX_AMOUNT_DOLLARS: i64 = 99_999_999;

/// The largest total sent over IPC: JavaScript's `Number.MAX_SAFE_INTEGER`
/// (2^53 - 1), so a sum never reaches the web view as a rounded number. With
/// every amount within [`MAX_AMOUNT_DOLLARS`] no real collection comes near
/// it; it only bounds rows an older database or a future path may have left
/// above the cap.
pub const MAX_TOTAL_DOLLARS: i64 = (1 << 53) - 1;

/// Adds amounts without ever failing or wrapping (#67): the sum saturates at
/// [`MAX_TOTAL_DOLLARS`], so one oversized row can neither make a listing
/// error out nor wrap into a wrong total. A negative stored value (which the
/// schema's CHECK forbids) counts as 0.
pub fn saturating_total(amounts: impl IntoIterator<Item = i64>) -> i64 {
    let total = amounts.into_iter().fold(0i128, |sum, amount| sum + i128::from(amount.max(0)));
    i64::try_from(total).map_or(MAX_TOTAL_DOLLARS, |total| total.min(MAX_TOTAL_DOLLARS))
}

/// `a + b` for two totals, saturating as [`saturating_total`] does.
pub fn saturating_add(a: i64, b: i64) -> i64 {
    saturating_total([a, b])
}

/// FR-037: an amount is a whole number of dollars, so once it has decoded
/// (a fractional number never does) the only things left to refuse are a
/// negative one and one above [`MAX_AMOUNT_DOLLARS`] (#67).
pub(crate) fn checked_amount(
    field: &str,
    label: &str,
    value: Option<i64>,
    errors: &mut FieldErrors,
) {
    match value {
        Some(dollars) if dollars < 0 => {
            errors.insert(field.into(), format!("{label} can't be negative."));
        }
        Some(dollars) if dollars > MAX_AMOUNT_DOLLARS => {
            errors.insert(field.into(), format!("{label} can't be more than $99,999,999."));
        }
        _ => {}
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
}

/// FR-003/FR-004: both dates are judged against the user's local date, and
/// the disposition date is not before the acquisition date. A disposed
/// record needs its type, recipient and date, and an active one none of
/// them. The price is optional (research.md §9): a record disposed of along
/// with its host may have none.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(5_000_000), "5,000,000");
        assert_eq!(thousands(32_767), "32,767");
    }
}
